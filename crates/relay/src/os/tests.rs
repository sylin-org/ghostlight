//! Tests of the platform module: a detached child holds nothing of its starter, a program that
//! cannot start is an error, and (Windows) command-line quoting and process identity.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::session::EXCLUSIVE;
use super::spawn_detached;
use crate::tests::Scratch;

/// An existing directory for a child to start in. The tests never write there.
const START_DIRECTORY: &str = env!("CARGO_MANIFEST_DIR");

/// How long a reader of the starter's pipe may wait for end of file.
const END_OF_FILE_WITHIN: Duration = Duration::from_secs(2);

#[cfg(windows)]
use windows_platform as platform;

#[cfg(target_os = "linux")]
use linux_platform as platform;

#[cfg(windows)]
mod windows_platform {
    //! Windows pieces of the inheritance test: an inheritable pipe, `ping` as the long-lived
    //! program and `TerminateProcess` to end it.

    use std::ffi::{c_void, OsString};
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;
    use std::path::PathBuf;
    use std::ptr::null_mut;

    use crate::os::{process_identity, process_is_alive};

    type Handle = *mut c_void;

    const PROCESS_TERMINATE: u32 = 0x0001;

    #[repr(C)]
    struct SecurityAttributes {
        length: u32,
        descriptor: *mut c_void,
        inherit: i32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreatePipe(
            read: *mut Handle,
            write: *mut Handle,
            attributes: *const SecurityAttributes,
            size: u32,
        ) -> i32;
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn TerminateProcess(process: Handle, exit_code: u32) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    /// A pipe whose two ends a child created with inheritance on would receive.
    pub fn inheritable_pipe() -> (File, File) {
        let attributes = SecurityAttributes {
            length: std::mem::size_of::<SecurityAttributes>() as u32,
            descriptor: null_mut(),
            inherit: 1,
        };
        let (mut read, mut write) = (null_mut(), null_mut());
        // SAFETY: both out-pointers and the attributes are valid for the call.
        let created = unsafe { CreatePipe(&mut read, &mut write, &attributes, 0) };
        assert_ne!(created, 0, "CreatePipe failed");
        // SAFETY: CreatePipe returned two open handles, and each is owned by exactly one File.
        unsafe { (File::from_raw_handle(read), File::from_raw_handle(write)) }
    }

    /// An ordinary Windows program that runs for about five seconds.
    pub fn long_lived_program() -> (PathBuf, Vec<OsString>) {
        let root = std::env::var_os("SystemRoot").expect("SystemRoot is set");
        let program = PathBuf::from(root).join("System32").join("PING.EXE");
        let arguments = ["-n", "6", "127.0.0.1"].map(OsString::from).to_vec();
        (program, arguments)
    }

    /// Whether the process `pid` is running.
    pub fn is_running(pid: u32) -> bool {
        process_identity(pid).is_some_and(process_is_alive)
    }

    /// Ends the process `pid` when dropped, so a failed assertion leaves nothing running.
    pub struct EndOnDrop(pub u32);

    impl Drop for EndOnDrop {
        fn drop(&mut self) {
            // SAFETY: OpenProcess takes a numeric id; the handle is closed once below.
            let process = unsafe { OpenProcess(PROCESS_TERMINATE, 0, self.0) };
            if !process.is_null() {
                // SAFETY: process is a valid handle with terminate access, owned here.
                unsafe {
                    TerminateProcess(process, 1);
                    CloseHandle(process);
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_platform {
    //! Linux pieces of the inheritance test: a pipe without close-on-exec, `sleep` as the
    //! long-lived program and `kill` to end it.

    use std::ffi::{c_int, OsString};
    use std::fs::File;
    use std::os::unix::io::FromRawFd;
    use std::path::{Path, PathBuf};

    const SIGKILL: c_int = 9;

    extern "C" {
        fn pipe(descriptors: *mut c_int) -> c_int;
        fn kill(process_id: c_int, signal: c_int) -> c_int;
    }

    /// A pipe whose two ends a child that kept all its descriptors would receive.
    pub fn inheritable_pipe() -> (File, File) {
        let mut descriptors = [0 as c_int; 2];
        // SAFETY: descriptors has room for the two descriptors pipe writes.
        let created = unsafe { pipe(descriptors.as_mut_ptr()) };
        assert_eq!(created, 0, "pipe failed");
        // SAFETY: pipe returned two open descriptors, and each is owned by exactly one File.
        unsafe {
            (
                File::from_raw_fd(descriptors[0]),
                File::from_raw_fd(descriptors[1]),
            )
        }
    }

    /// An ordinary Linux program that runs for five seconds.
    pub fn long_lived_program() -> (PathBuf, Vec<OsString>) {
        (PathBuf::from("/bin/sleep"), vec![OsString::from("5")])
    }

    /// Whether the process `pid` still exists.
    pub fn is_running(pid: u32) -> bool {
        Path::new(&format!("/proc/{pid}")).exists()
    }

    /// Ends the process `pid` when dropped, so a failed assertion leaves nothing running.
    pub struct EndOnDrop(pub u32);

    impl Drop for EndOnDrop {
        fn drop(&mut self) {
            // SAFETY: kill takes two integers and touches no memory.
            unsafe { kill(self.0 as c_int, SIGKILL) };
        }
    }
}

#[test]
fn a_detached_child_holds_none_of_its_starters_handles() {
    let _exclusive = EXCLUSIVE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let (reader, writer) = platform::inheritable_pipe();
    let (program, arguments) = platform::long_lived_program();
    let arguments: Vec<&OsStr> = arguments.iter().map(OsString::as_os_str).collect();
    let pid = spawn_detached(&program, &arguments, Path::new(START_DIRECTORY))
        .expect("the long-lived program starts");
    let _end = platform::EndOnDrop(pid);

    drop(writer);
    let (sent, received) = mpsc::channel();
    thread::spawn(move || {
        let mut reader: File = reader;
        let mut rest = Vec::new();
        let _ = sent.send(reader.read_to_end(&mut rest).map(|_| rest.len()));
    });
    let outcome = received
        .recv_timeout(END_OF_FILE_WITHIN)
        .expect("the pipe reaches end of file while the child runs, so the child does not hold it");
    assert_eq!(outcome.expect("the read ends cleanly"), 0);
    assert!(platform::is_running(pid), "the child is still running");
}

#[test]
fn a_program_that_cannot_start_is_an_error() {
    let _exclusive = EXCLUSIVE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let missing = Path::new("ghostlight-no-such-program");
    assert!(spawn_detached(missing, &[], Path::new(START_DIRECTORY)).is_err());
}

#[test]
fn a_directory_that_does_not_exist_is_an_error() {
    let _exclusive = EXCLUSIVE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let (program, arguments) = platform::long_lived_program();
    let arguments: Vec<&OsStr> = arguments.iter().map(OsString::as_os_str).collect();
    let scratch = Scratch::named("spawn");
    let missing: PathBuf = scratch.join("no-such-directory");
    assert!(spawn_detached(&program, &arguments, &missing).is_err());
}

#[cfg(windows)]
mod windows_only {
    //! Command-line quoting, creation flags and process identity.

    use std::ffi::OsStr;
    use std::path::Path;
    use std::process::Command;

    use crate::os::windows::{command_line, CREATION_FLAGS};
    use crate::os::{parent_process, process_identity, process_is_alive};

    fn line(executable: &str, arguments: &[&str]) -> String {
        let arguments: Vec<&OsStr> = arguments.iter().map(OsStr::new).collect();
        String::from_utf16(&command_line(Path::new(executable), &arguments)).unwrap()
    }

    #[test]
    fn plain_arguments_are_not_quoted() {
        assert_eq!(
            line(r"C:\g\ghostlight.exe", &["serve"]),
            r"C:\g\ghostlight.exe serve"
        );
        assert_eq!(line("g", &[r"C:\dir\"]), r"g C:\dir\");
    }

    #[test]
    fn an_empty_argument_is_quoted() {
        assert_eq!(line("g", &[""]), r#"g """#);
    }

    #[test]
    fn a_path_with_a_space_is_quoted() {
        assert_eq!(
            line(r"C:\Program Files\g\ghostlight.exe", &["a\tb"]),
            "\"C:\\Program Files\\g\\ghostlight.exe\" \"a\tb\""
        );
    }

    #[test]
    fn a_quote_inside_an_argument_is_escaped() {
        assert_eq!(line("g", &[r#"say "hi""#]), r#"g "say \"hi\"""#);
    }

    #[test]
    fn backslashes_before_a_quote_or_the_closing_quote_are_doubled() {
        assert_eq!(line("g", &[r#"a\"b"#]), r#"g "a\\\"b""#);
        assert_eq!(line("g", &[r"C:\My Dir\"]), r#"g "C:\My Dir\\""#);
        assert_eq!(line("g", &[r"a b\\c"]), r#"g "a b\\c""#);
    }

    #[test]
    fn the_launch_is_windowless_detached_and_in_its_own_group() {
        assert_eq!(
            CREATION_FLAGS,
            0x0800_0000 | 0x0000_0008 | 0x0000_0200 | 0x0000_0400
        );
    }

    #[test]
    fn the_current_process_has_a_live_stable_identity() {
        let process = process_identity(std::process::id()).expect("current process identity");
        assert!(process_is_alive(process));
        assert_eq!(process_identity(std::process::id()), Some(process));
    }

    #[test]
    fn a_test_process_has_a_parent_to_name() {
        let parent = parent_process().expect("parent process identity");
        assert_ne!(parent.process_id, std::process::id());
    }

    #[test]
    fn a_terminated_process_reads_as_dead_while_its_handle_is_held() {
        let mut child = Command::new("cmd")
            .args(["/C", "ping -n 30 127.0.0.1 >nul"])
            .spawn()
            .expect("spawn long-lived child");
        let process = process_identity(child.id()).expect("child process identity");
        assert!(process_is_alive(process));
        child.kill().expect("terminate child");
        child.wait().expect("reap child status");
        assert!(!process_is_alive(process));
    }
}

mod app_user_model_id {
    use std::io::ErrorKind;

    use crate::os::set_app_user_model_id;

    #[test]
    fn an_empty_id_is_invalid_input() {
        let error = set_app_user_model_id("").expect_err("empty id");
        assert_eq!(error.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn an_id_with_an_interior_nul_is_invalid_input() {
        let error = set_app_user_model_id("Sylin\0Ghostlight").expect_err("NUL in id");
        assert_eq!(error.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn a_valid_id_is_accepted() {
        set_app_user_model_id("Sylin.Ghostlight").expect("set the test process id");
    }
}

mod console_interrupts {
    use crate::os::{forward_console_interrupts, swallows_console_event};

    const CTRL_C_EVENT: u32 = 0;
    const CTRL_BREAK_EVENT: u32 = 1;
    const CTRL_CLOSE_EVENT: u32 = 2;
    const CTRL_LOGOFF_EVENT: u32 = 5;
    const CTRL_SHUTDOWN_EVENT: u32 = 6;

    #[test]
    fn only_ctrl_c_and_ctrl_break_are_swallowed() {
        assert!(swallows_console_event(CTRL_C_EVENT));
        assert!(swallows_console_event(CTRL_BREAK_EVENT));
        for event in [
            CTRL_CLOSE_EVENT,
            CTRL_LOGOFF_EVENT,
            CTRL_SHUTDOWN_EVENT,
            3,
            4,
            7,
            u32::MAX,
        ] {
            assert!(!swallows_console_event(event), "event {event}");
        }
    }

    #[test]
    fn registering_twice_is_harmless() {
        forward_console_interrupts().expect("first registration");
        forward_console_interrupts().expect("second registration");
    }
}
