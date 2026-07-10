// SPDX-License-Identifier: Apache-2.0 OR MIT
//! gif_creator recording sessions (ADR-0053 Decisions 3 and 4): the service-side owner of every
//! recording's state and frames.
//!
//! One [`RecordingStore`] hangs off the [`super::browser::Browser`]. Frames arrive as unsolicited
//! `gif_frame` events from the extension's screencast relay (already interval-thinned at the
//! source) and are written to disk under a per-tab directory -- the big payload never sits in
//! memory. Session metadata (active flag, per-frame timestamps/viewport/action tags, the pending
//! action queue) lives in memory: the service is a normal process, which IS the resilience story
//! (ADR-0053 Consequences); a startup sweep purges any frames a previous process left behind.
//! Action tagging keeps ADR-0052 D4's semantics: each noted action tags the first kept frame at or
//! after its timestamp.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::b64;
use crate::gif::{take_action_for_frame, ActionMeta, RecordedFrame};

/// Bound on stored frames per recording; the oldest frame is evicted past it (matches the
/// extension-era cap).
const MAX_FRAMES: usize = 100;

/// Bound on the pending action queue: an action this far behind the kept-frame stream will never
/// tag a frame.
const PENDING_ACTION_BOUND: usize = 20;

/// Per-frame metadata mirrored in memory; the JPEG bytes live at `<root>/<tabId>/<seq>.jpg`.
struct FrameMeta {
    seq: u64,
    ts_ms: i64,
    vp_w: Option<f64>,
    action: Option<ActionMeta>,
}

struct Session {
    active: bool,
    next_seq: u64,
    vp_w: Option<f64>,
    frames: Vec<FrameMeta>,
    pending: Vec<ActionMeta>,
}

/// The per-tab recording sessions plus their on-disk frame directory.
pub struct RecordingStore {
    root: PathBuf,
    sessions: Mutex<HashMap<i64, Session>>,
}

impl RecordingStore {
    /// A store rooted in the OS temp directory, namespaced per instance label (ADR-0044). Sweeps
    /// any frames a previous service process left behind.
    pub(crate) fn new() -> Self {
        let label = ghostlight_transport::instance::Instance::resolve()
            .label()
            .to_string();
        let root = std::env::temp_dir()
            .join(format!("ghostlight-{label}"))
            .join("recordings");
        Self::with_root(root)
    }

    /// A store rooted at `root` (tests inject a scratch dir). Best-effort startup sweep.
    pub(crate) fn with_root(root: PathBuf) -> Self {
        let _ = fs::remove_dir_all(&root);
        RecordingStore {
            root,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    fn tab_dir(&self, tab: i64) -> PathBuf {
        self.root.join(tab.to_string())
    }

    fn frame_path(&self, tab: i64, seq: u64) -> PathBuf {
        self.tab_dir(tab).join(format!("{seq}.jpg"))
    }

    /// Begin (or restart) a recording for `tab`: prior frames are discarded.
    pub(crate) fn start(&self, tab: i64) {
        let _ = fs::remove_dir_all(self.tab_dir(tab));
        self.sessions.lock().unwrap().insert(
            tab,
            Session {
                active: true,
                next_seq: 0,
                vp_w: None,
                frames: Vec::new(),
                pending: Vec::new(),
            },
        );
    }

    /// Record the viewport width probed at start (per-frame metadata overrides when present).
    pub(crate) fn set_vp_w(&self, tab: i64, vp_w: f64) {
        if let Some(s) = self.sessions.lock().unwrap().get_mut(&tab) {
            s.vp_w = Some(vp_w);
        }
    }

    /// Whether `tab` has an ACTIVE recording (the hot-path guard for action noting).
    pub(crate) fn is_active(&self, tab: i64) -> bool {
        self.sessions
            .lock()
            .unwrap()
            .get(&tab)
            .is_some_and(|s| s.active)
    }

    /// Note a dispatched action for overlay tagging (ADR-0052 D4); no-op unless actively
    /// recording.
    pub(crate) fn note_action(&self, tab: i64, meta: ActionMeta) {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(s) = sessions.get_mut(&tab) else {
            return;
        };
        if !s.active {
            return;
        }
        s.pending.push(meta);
        while s.pending.len() > PENDING_ACTION_BOUND {
            s.pending.remove(0);
        }
    }

    /// Store one screencast frame (base64 JPEG) for an active recording: assign a sequence
    /// number, tag it with the action whose paint it shows, write the bytes to disk, and evict
    /// past the cap. Silently drops frames for inactive/unknown tabs or undecodable payloads.
    pub(crate) fn on_frame(&self, tab: i64, data_b64: &str, ts_ms: i64, device_width: Option<f64>) {
        let Some(bytes) = b64::decode(data_b64) else {
            return;
        };
        // Assign the seq + tag under the lock; do the file write outside it.
        let (seq, evict) = {
            let mut sessions = self.sessions.lock().unwrap();
            let Some(s) = sessions.get_mut(&tab) else {
                return;
            };
            if !s.active {
                return;
            }
            let seq = s.next_seq;
            s.next_seq += 1;
            let action = take_action_for_frame(&mut s.pending, ts_ms);
            s.frames.push(FrameMeta {
                seq,
                ts_ms,
                vp_w: device_width.or(s.vp_w),
                action,
            });
            let evict = if s.frames.len() > MAX_FRAMES {
                Some(s.frames.remove(0).seq)
            } else {
                None
            };
            (seq, evict)
        };
        let path = self.frame_path(tab, seq);
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if fs::write(&path, &bytes).is_err() {
            // Roll the meta back so frames() never references a missing file.
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(s) = sessions.get_mut(&tab) {
                s.frames.retain(|f| f.seq != seq);
            }
            return;
        }
        if let Some(old) = evict {
            let _ = fs::remove_file(self.frame_path(tab, old));
        }
    }

    /// Stop recording (frames kept). Returns the kept count, or None when no recording exists.
    pub(crate) fn stop(&self, tab: i64) -> Option<usize> {
        let mut sessions = self.sessions.lock().unwrap();
        let s = sessions.get_mut(&tab)?;
        s.active = false;
        Some(s.frames.len())
    }

    /// Discard `tab`'s recording entirely (session + on-disk frames).
    pub(crate) fn clear(&self, tab: i64) {
        self.sessions.lock().unwrap().remove(&tab);
        let _ = fs::remove_dir_all(self.tab_dir(tab));
    }

    /// The recording's frames in capture order, JPEG bytes re-read from disk (frames whose file
    /// vanished are skipped). Ready for [`crate::gif::encode_recording`].
    pub(crate) fn frames(&self, tab: i64) -> Vec<RecordedFrame> {
        let metas: Vec<(u64, i64, Option<f64>, Option<ActionMeta>)> = {
            let sessions = self.sessions.lock().unwrap();
            let Some(s) = sessions.get(&tab) else {
                return Vec::new();
            };
            s.frames
                .iter()
                .map(|f| (f.seq, f.ts_ms, f.vp_w, f.action.clone()))
                .collect()
        };
        metas
            .into_iter()
            .filter_map(|(seq, ts_ms, vp_w, action)| {
                let jpeg = fs::read(self.frame_path(tab, seq)).ok()?;
                Some(RecordedFrame {
                    jpeg,
                    ts_ms,
                    vp_w,
                    action,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("ghostlight-recording-tests")
            .join(name)
    }

    fn frame_b64(byte: u8) -> String {
        b64::encode(&[byte, byte, byte])
    }

    #[test]
    fn lifecycle_start_frame_stop_clear() {
        let store = RecordingStore::with_root(scratch("lifecycle"));
        assert!(!store.is_active(1));
        store.start(1);
        assert!(store.is_active(1));
        store.on_frame(1, &frame_b64(0xAA), 1000, Some(800.0));
        store.on_frame(1, &frame_b64(0xBB), 1300, Some(800.0));
        assert_eq!(store.stop(1), Some(2));
        assert!(!store.is_active(1), "stopped keeps the session, inactive");
        // Frames read back in order with their bytes and metadata.
        let frames = store.frames(1);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].jpeg, vec![0xAA, 0xAA, 0xAA]);
        assert_eq!(frames[1].ts_ms, 1300);
        assert_eq!(frames[0].vp_w, Some(800.0));
        // Frames arriving after stop are dropped.
        store.on_frame(1, &frame_b64(0xCC), 1400, None);
        assert_eq!(store.frames(1).len(), 2);
        store.clear(1);
        assert!(store.frames(1).is_empty());
        assert_eq!(store.stop(1), None, "cleared session is gone");
    }

    #[test]
    fn restart_discards_prior_frames_and_actions_tag_frames() {
        let store = RecordingStore::with_root(scratch("restart-tag"));
        store.start(7);
        store.on_frame(7, &frame_b64(1), 100, None);
        store.start(7);
        assert!(store.frames(7).is_empty(), "restart discards prior frames");

        // ADR-0052 D4: the action tags the FIRST kept frame at-or-after its timestamp.
        store.note_action(
            7,
            ActionMeta {
                kind: "left_click".into(),
                coordinate: Some((10.0, 20.0)),
                start_coordinate: None,
                description: "left_click".into(),
                ts_ms: 500,
            },
        );
        store.on_frame(7, &frame_b64(2), 400, None); // before the action: untagged
        store.on_frame(7, &frame_b64(3), 600, None); // first at-or-after: tagged
        store.on_frame(7, &frame_b64(4), 800, None); // queue drained: untagged
        let frames = store.frames(7);
        assert!(frames[0].action.is_none());
        assert_eq!(frames[1].action.as_ref().unwrap().kind, "left_click");
        assert!(frames[2].action.is_none());
    }

    #[test]
    fn eviction_keeps_the_last_max_frames() {
        let store = RecordingStore::with_root(scratch("evict"));
        store.start(3);
        for i in 0..(MAX_FRAMES + 5) {
            store.on_frame(3, &frame_b64(i as u8), i as i64, None);
        }
        let frames = store.frames(3);
        assert_eq!(frames.len(), MAX_FRAMES);
        assert_eq!(frames[0].ts_ms, 5, "the oldest five were evicted");
        // Evicted files are actually gone from disk.
        assert!(!store.frame_path(3, 0).exists());
    }

    #[test]
    fn garbage_base64_and_unknown_tabs_are_dropped() {
        let store = RecordingStore::with_root(scratch("garbage"));
        store.on_frame(99, &frame_b64(1), 1, None); // no session
        store.start(99);
        store.on_frame(99, "not base64!", 2, None);
        assert!(store.frames(99).is_empty());
        // Actions on inactive/unknown tabs are no-ops.
        store.stop(99);
        store.note_action(
            99,
            ActionMeta {
                kind: "type".into(),
                coordinate: None,
                start_coordinate: None,
                description: "x".into(),
                ts_ms: 1,
            },
        );
    }
}
