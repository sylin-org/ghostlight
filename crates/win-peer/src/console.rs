//! Joining the console of the parent process, through the audited FFI boundary.

#[link(name = "kernel32")]
extern "system" {
    fn AttachConsole(process_id: u32) -> i32;
}

/// `ATTACH_PARENT_PROCESS`: the console of the process that started this one.
const ATTACH_PARENT_PROCESS: u32 = u32::MAX;

/// Attach to the parent process's console; every failure is ignored.
pub(crate) fn attach_parent() {
    // SAFETY: AttachConsole takes no pointers. A parent with no console, or a console this
    // process already has, makes it return zero, which leaves the process as it was.
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}
