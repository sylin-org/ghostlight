//! The live stream: `GET /api/events`, one numbered sequence of changes for every screen.
//!
//! An [`EventHub`] is the workbench's event sink for the loopback server. Each change is projected
//! once for each audience that has a subscriber, so a watcher never receives what only the window
//! may see, and is queued for every stream in a bounded channel that publishing never waits on.
//! A stream that falls behind by more than the channel holds is ended with a `reset` event, and
//! its screen reads a fresh snapshot. Turning watching off ends every watching stream with a
//! `closed` event that says why, and leaves the window's streams open (ADR-0199, amendment of
//! 2026-10-15).

use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde_json::json;

use super::http::{EventWriter, Incoming, Response};
use crate::language::workbench as words;
use crate::workbench::context::WebContext;
use crate::workbench::parts::{Audience, Projection};
use crate::workbench::{WorkbenchChange, WorkbenchEvent, WorkbenchEventSink};

/// The type of the stream.
pub const EVENT_STREAM_TYPE: &str = "text/event-stream";

/// The most changes one stream queues before it is ended.
pub const QUEUE_DEPTH: usize = 256;

/// The most streams the window may have open at once.
pub const MAX_CONTROL_STREAMS: usize = 4;

/// The most streams watching screens may have open at once.
pub const MAX_WATCH_STREAMS: usize = 8;

/// How long a stream stays quiet before it says it is still there.
pub const KEEP_INTERVAL: Duration = Duration::from_secs(15);

/// How long a write to a screen may take before the stream gives up on it.
pub const WRITE_DEADLINE: Duration = Duration::from_secs(5);

/// The name of the event that opens a stream.
const EVENT_HELLO: &str = "hello";

/// The name of the event that ends a stream whose screen must read a snapshot again.
const EVENT_RESET: &str = "reset";

/// The name of the event that ends a watching stream because watching was turned off.
const EVENT_CLOSED: &str = "closed";

/// The comment a quiet stream sends.
const COMMENT_KEEP: &str = ": keep";

/// Why a stream was reset: its screen did not keep up.
const RESET_BEHIND: &str = "behind";

/// Every place for a live view of this kind is taken.
#[derive(Debug, Eq, PartialEq)]
pub struct TooManyScreens;

/// One stream's place in the hub.
struct Subscriber {
    id: u64,
    context: WebContext,
    /// The stream carries only changes numbered above this.
    after: u64,
    queue: SyncSender<Arc<str>>,
    behind: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
}

/// Fans every published change out to every open stream.
pub struct EventHub {
    instance: String,
    controller: Projection,
    watcher: Projection,
    subscribers: Mutex<Vec<Subscriber>>,
    next_id: AtomicU64,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl EventHub {
    /// A hub whose streams name changes by `instance` and their number.
    #[must_use]
    pub fn new(instance: String) -> Self {
        Self {
            instance,
            controller: Projection::new(Audience::Control),
            watcher: Projection::new(Audience::Watch),
            subscribers: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(0),
        }
    }

    /// Open a stream for `context`, which must be Control or Watch.
    ///
    /// `seq` reads the current sequence while the hub is locked, so the stream carries exactly the
    /// changes numbered above the one its hello names.
    pub fn open(
        self: &Arc<Self>,
        context: WebContext,
        seq: impl FnOnce() -> u64,
    ) -> Result<Subscription, TooManyScreens> {
        let limit = match context {
            WebContext::Control => MAX_CONTROL_STREAMS,
            WebContext::Watch => MAX_WATCH_STREAMS,
            WebContext::Closed => return Err(TooManyScreens),
        };
        let mut subscribers = lock(&self.subscribers);
        if subscribers
            .iter()
            .filter(|subscriber| subscriber.context == context)
            .count()
            >= limit
        {
            return Err(TooManyScreens);
        }
        let after = seq();
        let id = self.next_id.fetch_add(1, Ordering::AcqRel);
        let (queue, receiver) = mpsc::sync_channel(QUEUE_DEPTH);
        let behind = Arc::new(AtomicBool::new(false));
        let closed = Arc::new(AtomicBool::new(false));
        subscribers.push(Subscriber {
            id,
            context,
            after,
            queue,
            behind: Arc::clone(&behind),
            closed: Arc::clone(&closed),
        });
        Ok(Subscription {
            hub: Arc::clone(self),
            id,
            context,
            hello_seq: after,
            receiver,
            behind,
            closed,
        })
    }

    /// End every stream without a reset. A server that stops does this.
    pub fn close(&self) {
        lock(&self.subscribers).clear();
    }

    /// How many streams are open for `context`.
    #[cfg(test)]
    pub fn open_streams(&self, context: WebContext) -> usize {
        lock(&self.subscribers)
            .iter()
            .filter(|subscriber| subscriber.context == context)
            .count()
    }

    fn remove(&self, id: u64) {
        lock(&self.subscribers).retain(|subscriber| subscriber.id != id);
    }

    /// The frame one change makes for one audience, ready to write.
    fn frame(&self, audience: Audience, event: &WorkbenchEvent) -> Arc<str> {
        let projection = match audience {
            Audience::Control => &self.controller,
            Audience::Watch => &self.watcher,
        };
        let data = projection.event(event);
        format!("id: {}-{}\ndata: {}\n\n", self.instance, event.seq, data).into()
    }
}

impl WorkbenchEventSink for EventHub {
    fn publish(&self, event: WorkbenchEvent) {
        let mut subscribers = lock(&self.subscribers);
        let mut control: Option<Arc<str>> = None;
        let mut watch: Option<Arc<str>> = None;
        // A part that says watching is off ends every watching stream, and none of them is sent
        // the part: a screen that may no longer watch is shown no data.
        let watching_off = matches!(
            &event.change,
            WorkbenchChange::ControlsChanged { controls } if !controls.viewing.watch
        );
        subscribers.retain(|subscriber| {
            if subscriber.after >= event.seq {
                return true;
            }
            if watching_off && subscriber.context == WebContext::Watch {
                subscriber.closed.store(true, Ordering::Release);
                return false;
            }
            let frame = match subscriber.context {
                WebContext::Control => {
                    control.get_or_insert_with(|| self.frame(Audience::Control, &event))
                }
                WebContext::Watch => {
                    watch.get_or_insert_with(|| self.frame(Audience::Watch, &event))
                }
                WebContext::Closed => return false,
            };
            match subscriber.queue.try_send(Arc::clone(frame)) {
                Ok(()) => true,
                Err(TrySendError::Full(_)) => {
                    subscriber.behind.store(true, Ordering::Release);
                    false
                }
                Err(TrySendError::Disconnected(_)) => false,
            }
        });
    }
}

/// One open stream's end of the hub. Dropping it frees its place.
pub struct Subscription {
    hub: Arc<EventHub>,
    id: u64,
    context: WebContext,
    hello_seq: u64,
    receiver: Receiver<Arc<str>>,
    behind: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
}

impl std::fmt::Debug for Subscription {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Subscription")
            .field("id", &self.id)
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.hub.remove(self.id);
    }
}

/// What a stream writes next.
#[derive(Debug, Eq, PartialEq)]
pub enum Frame {
    /// One numbered change, already formatted.
    Change(Arc<str>),
    /// Nothing happened for a while.
    Keep,
    /// The screen fell behind and must read a snapshot again.
    Reset,
    /// Watching was turned off, so this watching stream is over.
    Closed,
    /// The stream is over.
    End,
}

impl Subscription {
    /// The event that opens the stream: the context, this start's instance and the sequence the
    /// stream continues from.
    #[must_use]
    pub fn hello(&self) -> String {
        let data = json!({
            "context": self.context.token(),
            "instance": self.hub.instance,
            "seq": self.hello_seq,
        });
        format!("event: {EVENT_HELLO}\ndata: {data}\n\n")
    }

    /// The next thing to write, waiting up to `keep` for a change.
    pub fn next(&self, keep: Duration) -> Frame {
        match self.receiver.recv_timeout(keep) {
            Ok(frame) => Frame::Change(frame),
            Err(RecvTimeoutError::Timeout) => Frame::Keep,
            Err(RecvTimeoutError::Disconnected) => {
                if self.closed.load(Ordering::Acquire) {
                    Frame::Closed
                } else if self.behind.load(Ordering::Acquire) {
                    Frame::Reset
                } else {
                    Frame::End
                }
            }
        }
    }
}

/// The text of the keep-alive comment.
#[must_use]
pub fn keep_frame() -> String {
    format!("{COMMENT_KEEP}\n\n")
}

/// The text of the event that tells a screen it fell behind.
#[must_use]
pub fn reset_frame() -> String {
    let data = json!({ "reason": RESET_BEHIND });
    format!("event: {EVENT_RESET}\ndata: {data}\n\n")
}

/// The text of the event that tells a watching screen watching was turned off.
#[must_use]
pub fn closed_frame() -> String {
    let data = json!({ "sentence": words::WATCHING_OFF });
    format!("event: {EVENT_CLOSED}\ndata: {data}\n\n")
}

/// Write `head` and then the stream until it ends, a write fails or a write takes too long.
pub fn serve(incoming: Incoming, head: &Response, subscription: &Subscription) {
    let Ok(mut writer) = incoming.begin_stream(head, WRITE_DEADLINE) else {
        return;
    };
    // A write that fails means the screen went away or stopped reading. Either way the stream is
    // over, and its place is freed when the subscription drops.
    let _ = write_stream(&mut writer, subscription, KEEP_INTERVAL);
}

fn write_stream(
    writer: &mut EventWriter,
    subscription: &Subscription,
    keep: Duration,
) -> io::Result<()> {
    writer.write(&subscription.hello())?;
    loop {
        match subscription.next(keep) {
            Frame::Change(frame) => writer.write(&frame)?,
            Frame::Keep => writer.write(&keep_frame())?,
            Frame::Reset => return writer.write(&reset_frame()),
            Frame::Closed => return writer.write(&closed_frame()),
            Frame::End => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::adapter::RuntimeControlState;
    use crate::workbench::{WorkbenchChange, WorkbenchEvent};

    fn change(seq: u64) -> WorkbenchEvent {
        WorkbenchEvent {
            seq,
            change: WorkbenchChange::RuntimeChanged {
                runtime_state: RuntimeControlState::Held,
            },
        }
    }

    fn hub() -> Arc<EventHub> {
        Arc::new(EventHub::new("instance_test".into()))
    }

    #[test]
    fn a_stream_carries_only_what_comes_after_its_hello() {
        let hub = hub();
        let stream = hub.open(WebContext::Control, || 3).unwrap();
        assert!(stream.hello().contains("\"seq\":3"));
        hub.publish(change(3));
        hub.publish(change(4));
        let Frame::Change(frame) = stream.next(Duration::from_millis(50)) else {
            panic!("the change after the hello is queued");
        };
        assert!(frame.starts_with("id: instance_test-4\ndata: "), "{frame}");
        assert!(frame.ends_with("\n\n"));
        assert_eq!(stream.next(Duration::from_millis(20)), Frame::Keep);
    }

    #[test]
    fn a_quiet_stream_keeps_and_a_dropped_stream_frees_its_place() {
        let hub = hub();
        let stream = hub.open(WebContext::Watch, || 0).unwrap();
        assert_eq!(hub.open_streams(WebContext::Watch), 1);
        assert_eq!(stream.next(Duration::from_millis(10)), Frame::Keep);
        drop(stream);
        assert_eq!(hub.open_streams(WebContext::Watch), 0);
    }

    #[test]
    fn the_window_and_watchers_have_their_own_places() {
        let hub = hub();
        let mut control = Vec::new();
        for _ in 0..MAX_CONTROL_STREAMS {
            control.push(hub.open(WebContext::Control, || 0).unwrap());
        }
        assert_eq!(
            hub.open(WebContext::Control, || 0).err(),
            Some(TooManyScreens)
        );
        let mut watch = Vec::new();
        for _ in 0..MAX_WATCH_STREAMS {
            watch.push(hub.open(WebContext::Watch, || 0).unwrap());
        }
        assert_eq!(
            hub.open(WebContext::Watch, || 0).err(),
            Some(TooManyScreens)
        );
        drop(control.pop());
        assert!(hub.open(WebContext::Control, || 0).is_ok());
        assert_eq!(
            hub.open(WebContext::Closed, || 0).err(),
            Some(TooManyScreens)
        );
    }

    #[test]
    fn a_stream_that_falls_behind_is_reset_and_the_others_keep_going() {
        let hub = hub();
        let slow = hub.open(WebContext::Control, || 0).unwrap();
        let quick = hub.open(WebContext::Watch, || 0).unwrap();
        let mut quick_seen = 0;
        for seq in 1..=(QUEUE_DEPTH as u64 + 40) {
            hub.publish(change(seq));
            while let Frame::Change(_) = quick.next(Duration::ZERO) {
                quick_seen += 1;
            }
        }
        assert_eq!(quick_seen, QUEUE_DEPTH + 40);
        assert_eq!(hub.open_streams(WebContext::Control), 0);
        let mut queued = 0;
        loop {
            match slow.next(Duration::from_millis(10)) {
                Frame::Change(_) => queued += 1,
                Frame::Reset => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(queued, QUEUE_DEPTH);
        assert!(reset_frame().contains("\"reason\":\"behind\""));
    }

    #[test]
    fn closing_the_hub_ends_streams_without_a_reset() {
        let hub = hub();
        let stream = hub.open(WebContext::Control, || 0).unwrap();
        hub.close();
        assert_eq!(stream.next(Duration::from_millis(10)), Frame::End);
    }
}
