//! Tests of the platform module: a detached child holds nothing of its starter and a start that
//! cannot happen is an error, (Windows) command-line quoting and process identity, the
//! AppUserModelID call, and the ancestor walk.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Read;
use std::path::Path;
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

/// Defect 2c5f7954 and the audited process start: a detached child holds none of its starter's
/// handles or descriptors, so a starter whose output is captured (an installer piped through a
/// filter) sees that output end while the child still runs. A start that cannot happen, a
/// missing program or a missing directory, is still an error; on Linux that means the
/// close-on-exec marking left the standard library's exec-error pipe open until the exec.
#[test]
fn a_detached_start_holds_nothing_of_its_starter_and_reports_a_start_that_cannot_happen() {
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

    let missing = Path::new("ghostlight-no-such-program");
    assert!(
        spawn_detached(missing, &[], Path::new(START_DIRECTORY)).is_err(),
        "a missing program"
    );
    let scratch = Scratch::named("spawn");
    assert!(
        spawn_detached(&program, &arguments, &scratch.join("no-such-directory")).is_err(),
        "a missing directory"
    );
}

#[cfg(windows)]
mod windows_only {
    //! Command-line quoting and process identity.

    use std::ffi::OsStr;
    use std::path::Path;
    use std::process::Command;

    use crate::os::windows::command_line;
    use crate::os::{process_identity, process_is_alive};

    fn line(executable: &str, arguments: &[&str]) -> String {
        let arguments: Vec<&OsStr> = arguments.iter().map(OsStr::new).collect();
        String::from_utf16(&command_line(Path::new(executable), &arguments)).unwrap()
    }

    /// The audited process start on Windows: `CreateProcessW` takes one command line, which the
    /// child's C runtime splits again, so each argument is quoted by the rules that runtime
    /// parses. An argument that is empty or holds a space, a tab or a quote is quoted; a quote
    /// inside is escaped; the backslashes before a quote or before the closing quote are
    /// doubled; and every other argument, a trailing backslash included, passes as it is. A
    /// home whose path holds a space starts its service only because of these rules.
    #[test]
    fn the_command_line_is_quoted_as_the_c_runtime_parses_it() {
        let cases: [(&str, &[&str], &str); 8] = [
            (
                r"C:\g\ghostlight.exe",
                &["serve"],
                r"C:\g\ghostlight.exe serve",
            ),
            ("g", &[r"C:\dir\"], r"g C:\dir\"),
            ("g", &[""], r#"g """#),
            (
                r"C:\Program Files\g\ghostlight.exe",
                &["a\tb"],
                "\"C:\\Program Files\\g\\ghostlight.exe\" \"a\tb\"",
            ),
            ("g", &[r#"say "hi""#], r#"g "say \"hi\"""#),
            ("g", &[r#"a\"b"#], r#"g "a\\\"b""#),
            ("g", &[r"C:\My Dir\"], r#"g "C:\My Dir\\""#),
            ("g", &[r"a b\\c"], r#"g "a b\\c""#),
        ];
        for (executable, arguments, expected) in cases {
            assert_eq!(
                line(executable, arguments),
                expected,
                "{executable} {arguments:?}"
            );
        }
    }

    /// The audited process identity on Windows, which the parent watch uses: a process is bound
    /// to its creation time, so it reads alive with a stable identity while it runs, and dead
    /// once it has ended even while a handle to it is still held, as a starter's handle often is
    /// (the signalled-handle check of 36c7ec91).
    #[test]
    fn a_process_reads_alive_while_it_runs_and_dead_once_ended_though_its_handle_is_held() {
        let this = process_identity(std::process::id()).expect("this process's identity");
        assert!(process_is_alive(this));
        let mut child = Command::new("cmd")
            .args(["/C", "ping -n 30 127.0.0.1 >nul"])
            .spawn()
            .expect("spawn long-lived child");
        let process = process_identity(child.id()).expect("child process identity");
        assert!(process_is_alive(process));
        assert_eq!(
            process_identity(child.id()),
            Some(process),
            "the identity is stable"
        );
        child.kill().expect("terminate child");
        child.wait().expect("reap child status");
        assert!(
            !process_is_alive(process),
            "dead, though `child` still holds its handle"
        );
    }
}

mod app_user_model_id {
    use std::io::ErrorKind;

    use crate::os::set_app_user_model_id;

    /// The audited AppUserModelID call: an id that is empty or holds a NUL, which the call
    /// would cut short, is refused before the call, and a valid id is set for the process. On
    /// Linux the call does nothing, since the shell names its window itself.
    #[test]
    fn the_app_user_model_id_call_sets_a_valid_id_and_refuses_one_it_would_cut_short() {
        for id in ["", "Sylin\0Ghostlight"] {
            assert_eq!(
                set_app_user_model_id(id)
                    .expect_err("an id the call would cut short")
                    .kind(),
                ErrorKind::InvalidInput,
                "{id:?}"
            );
        }
        set_app_user_model_id(crate::APP_USER_MODEL_ID).expect("a valid id is set");
    }
}

mod ancestry {
    //! The ancestor walk over synthetic process trees, and over this test's real ancestors.

    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::os::ancestors;
    use crate::os::ancestry::{walk, ProcessFacts};

    /// A synthetic process table: each id with its parent, its start time and its image.
    fn table(rows: &[(u32, Option<u32>, u64, Option<&str>)]) -> HashMap<u32, ProcessFacts> {
        rows.iter()
            .map(|(id, parent, started, image)| {
                (
                    *id,
                    ProcessFacts {
                        parent: *parent,
                        started: *started,
                        image: image.map(PathBuf::from),
                    },
                )
            })
            .collect()
    }

    /// The ancestors the walk finds above process 10, as (id, start time, image).
    fn walked(
        rows: &[(u32, Option<u32>, u64, Option<&str>)],
        limit: usize,
    ) -> Vec<(u32, u64, String)> {
        let table = table(rows);
        walk(10, limit, |id| table.get(&id).cloned())
            .into_iter()
            .map(|ancestor| {
                (
                    ancestor.process_id,
                    ancestor.started,
                    ancestor.image.to_string_lossy().into_owned(),
                )
            })
            .collect()
    }

    /// The images of [`walked`] alone.
    fn images(rows: &[(u32, Option<u32>, u64, Option<&str>)], limit: usize) -> Vec<String> {
        walked(rows, limit)
            .into_iter()
            .map(|(_, _, image)| image)
            .collect()
    }

    /// Defect 4f52e834 (one agent gets one set of tools, which matches a connection's source by
    /// its ancestor's id and start time) and ADR-0161: the ancestor walk reads each parent
    /// nearest first, with its id, start time and image, and stops at its limit, at the top, at
    /// a parent that is gone or cannot be read, and at a parent that started after its child,
    /// whose id therefore names a later process. A connection is never attributed to a process
    /// that reused an id.
    #[test]
    fn the_ancestor_walk_reads_parents_nearest_first_and_stops_where_the_chain_breaks() {
        let to_the_top = [
            (10, Some(20), 5, Some("edge")),
            (20, Some(30), 4, Some("shell")),
            (30, None, 1, Some("app")),
        ];
        assert_eq!(
            walked(&to_the_top, 16),
            [(20, 4, "shell".to_owned()), (30, 1, "app".to_owned())]
        );
        let long = [
            (10, Some(20), 5, Some("edge")),
            (20, Some(30), 4, Some("one")),
            (30, Some(40), 3, Some("two")),
            (40, None, 2, Some("three")),
        ];
        assert_eq!(images(&long, 2), ["one", "two"], "the limit");
        assert!(
            walked(&[(10, Some(20), 5, Some("edge"))], 16).is_empty(),
            "a parent that is gone"
        );
        let unreadable = [
            (10, Some(20), 5, Some("edge")),
            (20, Some(30), 4, Some("shell")),
            (30, Some(40), 3, None),
            (40, None, 2, Some("app")),
        ];
        assert_eq!(
            images(&unreadable, 16),
            ["shell"],
            "a parent whose image cannot be read"
        );
        assert!(walked(&[], 16).is_empty(), "the process itself is unknown");
        let reused = [
            (10, Some(20), 5, Some("edge")),
            (20, Some(30), 4, Some("shell")),
            (30, Some(10), 9, Some("later process")),
        ];
        assert_eq!(
            images(&reused, 16),
            ["shell"],
            "a parent started after its child is a reused id"
        );
    }

    /// The audited process reads behind the walk (on Windows the process snapshot,
    /// `GetProcessTimes` and `QueryFullProcessImageNameW`; on Linux `/proc`): this test process
    /// has real ancestors, each with an absolute path, read the same way twice, and a limit of
    /// zero reads none.
    #[test]
    fn this_test_process_has_real_ancestors_with_absolute_paths() {
        let found = ancestors(std::process::id(), 16);
        assert!(!found.is_empty(), "the test runner has a parent");
        assert!(found.len() <= 16);
        for ancestor in &found {
            assert!(ancestor.image.is_absolute(), "{}", ancestor.image.display());
        }
        assert_eq!(
            ancestors(std::process::id(), 16)[0],
            found[0],
            "the same parent reads as the same process"
        );
        assert!(ancestors(std::process::id(), 0).is_empty());
    }
}
