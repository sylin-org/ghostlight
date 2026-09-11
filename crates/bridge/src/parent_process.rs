//! Process identity used by thin-connector parent-death detection.

#[cfg(target_os = "linux")]
use std::fs;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParentProcess {
    process_id: u32,
    created: u64,
}

impl ParentProcess {
    pub(crate) fn capture() -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            let process = ghostlight_win_peer::parent_process()?;
            Some(Self {
                process_id: process.process_id,
                created: process.created,
            })
        }
        #[cfg(target_os = "linux")]
        {
            let parent_id = read_stat("self")?.parent_process_id;
            identity(parent_id)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        None
    }

    pub(crate) fn is_alive(self) -> bool {
        #[cfg(target_os = "windows")]
        {
            ghostlight_win_peer::process_is_alive(ghostlight_win_peer::ProcessIdentity {
                process_id: self.process_id,
                created: self.created,
            })
        }
        #[cfg(target_os = "linux")]
        {
            let Some(current) = read_stat("self") else {
                return true;
            };
            current.parent_process_id == self.process_id && identity(self.process_id) == Some(self)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        true
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug)]
struct ProcessStat {
    state: char,
    parent_process_id: u32,
    created: u64,
}

#[cfg(target_os = "linux")]
fn identity(process_id: u32) -> Option<ParentProcess> {
    let stat = read_stat(&process_id.to_string())?;
    (stat.state != 'Z').then_some(ParentProcess {
        process_id,
        created: stat.created,
    })
}

#[cfg(target_os = "linux")]
fn read_stat(process: &str) -> Option<ProcessStat> {
    let line = fs::read_to_string(format!("/proc/{process}/stat")).ok()?;
    let fields: Vec<_> = line
        .get(line.rfind(')')? + 1..)?
        .split_whitespace()
        .collect();
    Some(ProcessStat {
        state: fields.first()?.chars().next()?,
        parent_process_id: fields.get(1)?.parse().ok()?,
        created: fields.get(19)?.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::ParentProcess;

    #[test]
    fn current_parent_has_a_live_stable_identity() {
        let parent = ParentProcess::capture().expect("capture current parent");
        assert!(parent.is_alive());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn a_reaped_process_does_not_match_its_captured_identity() {
        use std::process::Command;

        let mut child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn long-lived child");
        let process = super::identity(child.id()).expect("child process identity");
        assert!(process.is_alive());
        child.kill().expect("terminate child");
        child.wait().expect("reap child status");
        assert!(!process.is_alive());
    }
}
