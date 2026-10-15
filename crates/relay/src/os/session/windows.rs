//! Windows session end: a hidden top-level window and the console control handler.
//!
//! When Windows signs out, shuts down or restarts, it broadcasts `WM_QUERYENDSESSION` and
//! `WM_ENDSESSION` to every top-level window of the desktop session and sends console events to
//! console processes. A process that loads `user32.dll`, as this module does, can be classified
//! as a GUI application and receive the window messages instead of `CTRL_LOGOFF_EVENT` and
//! `CTRL_SHUTDOWN_EVENT`, so both routes are listened to. The window is top-level, never
//! message-only, because a message-only window receives no broadcasts, and it is never visible.

use std::io;
use std::sync::mpsc::sync_channel;
use std::sync::Once;
use std::thread::{self, JoinHandle};

use super::{dispatch, SessionEvent};

pub(super) type Hwnd = *mut core::ffi::c_void;
type Hinstance = *mut core::ffi::c_void;
type Hicon = *mut core::ffi::c_void;
type Hcursor = *mut core::ffi::c_void;
type Hbrush = *mut core::ffi::c_void;
type Hmenu = *mut core::ffi::c_void;
pub(super) type Wparam = usize;
pub(super) type Lparam = isize;
pub(super) type Lresult = isize;

const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_QUERYENDSESSION: u32 = 0x0011;
const WM_ENDSESSION: u32 = 0x0016;

/// `GetSystemMetrics` index: nonzero while the current session is shutting down.
const SM_SHUTTINGDOWN: i32 = 0x2000;

const WS_POPUP: u32 = 0x8000_0000;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;

const CTRL_LOGOFF_EVENT: u32 = 5;
const CTRL_SHUTDOWN_EVENT: u32 = 6;

/// What the window procedure answers to a query: the session may end.
const ALLOW_END: Lresult = 1;

/// What the console handler answers: the event was not handled, so the default exit runs.
const NOT_HANDLED: i32 = 0;

/// The prefix of the window class name; the process and registration identities follow.
const CLASS_PREFIX: &str = "GhostlightSessionListener";

#[repr(C)]
struct WndClassW {
    style: u32,
    lpfn_wnd_proc: unsafe extern "system" fn(Hwnd, u32, Wparam, Lparam) -> Lresult,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: Hinstance,
    h_icon: Hicon,
    h_cursor: Hcursor,
    hbr_background: Hbrush,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
}

#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Default)]
struct Msg {
    hwnd: Hwnd,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt: Point,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(class: *const WndClassW) -> u16;
    fn UnregisterClassW(class_name: *const u16, instance: Hinstance) -> i32;
    fn CreateWindowExW(
        dw_ex_style: u32,
        lp_class_name: *const u16,
        lp_window_name: *const u16,
        dw_style: u32,
        x: i32,
        y: i32,
        n_width: i32,
        n_height: i32,
        h_wnd_parent: Hwnd,
        h_menu: Hmenu,
        h_instance: Hinstance,
        lp_param: *mut core::ffi::c_void,
    ) -> Hwnd;
    fn DestroyWindow(hwnd: Hwnd) -> i32;
    fn DefWindowProcW(hwnd: Hwnd, msg: u32, w_param: Wparam, l_param: Lparam) -> Lresult;
    fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, filter_min: u32, filter_max: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> Lresult;
    fn PostMessageW(hwnd: Hwnd, msg: u32, w_param: Wparam, l_param: Lparam) -> i32;
    fn PostQuitMessage(exit_code: i32);
    fn GetSystemMetrics(index: i32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(module_name: *const u16) -> Hinstance;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
}

/// Whether the system says the current session is shutting down. A process born after the
/// broadcast never hears it, and this answers for it.
pub(super) fn system_ending() -> bool {
    // SAFETY: GetSystemMetrics takes an index and no pointers.
    unsafe { GetSystemMetrics(SM_SHUTTINGDOWN) != 0 }
}

/// The window procedure: the query is allowed at once, the end is dispatched, and the window
/// closes when asked.
unsafe extern "system" fn window_proc(
    hwnd: Hwnd,
    msg: u32,
    w_param: Wparam,
    l_param: Lparam,
) -> Lresult {
    match msg {
        WM_QUERYENDSESSION => {
            dispatch(SessionEvent::Query);
            ALLOW_END
        }
        WM_ENDSESSION => {
            dispatch(if w_param != 0 {
                SessionEvent::Terminating
            } else {
                SessionEvent::Cancelled
            });
            0
        }
        WM_CLOSE => {
            // SAFETY: DestroyWindow is called on the valid window owned by this message loop.
            let _ = DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            // SAFETY: PostQuitMessage posts WM_QUIT to this thread's queue to end GetMessageW.
            PostQuitMessage(0);
            0
        }
        // SAFETY: unhandled messages go to DefWindowProcW with their original parameters.
        _ => DefWindowProcW(hwnd, msg, w_param, l_param),
    }
}

/// The console control handler. A logoff or shutdown is the final signal. Every event, close
/// included, is left unhandled so the default exit is never suppressed: answering TRUE would end
/// the default `ExitProcess` and could leave a process alive if its own stop stalls.
pub(super) unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> i32 {
    if ctrl_type == CTRL_SHUTDOWN_EVENT || ctrl_type == CTRL_LOGOFF_EVENT {
        dispatch(SessionEvent::Terminating);
    }
    NOT_HANDLED
}

/// Register the console control handler once per process.
fn register_console_handler() {
    static REGISTERED: Once = Once::new();
    REGISTERED.call_once(|| {
        // SAFETY: SetConsoleCtrlHandler registers a valid system callback.
        unsafe {
            SetConsoleCtrlHandler(Some(console_ctrl_handler), 1);
        }
    });
}

/// The window thread and its window, kept for as long as the listener lives.
pub(super) struct Attached {
    /// The listener window, which tests send messages to.
    pub(super) hwnd: Hwnd,
    thread: Option<JoinHandle<()>>,
}

// SAFETY: the window handle is only used to post WM_CLOSE from another thread, which the system
// allows, and the join handle moves between threads freely.
unsafe impl Send for Attached {}
// SAFETY: see above; nothing is mutated through a shared reference.
unsafe impl Sync for Attached {}

impl Drop for Attached {
    fn drop(&mut self) {
        if !self.hwnd.is_null() {
            // SAFETY: PostMessageW posts WM_CLOSE to the listener window, which closes it and
            // ends its message loop.
            unsafe {
                PostMessageW(self.hwnd, WM_CLOSE, 0, 0);
            }
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Create the hidden window on its own message-loop thread and register the console handler.
pub(super) fn attach(registration: usize) -> io::Result<Attached> {
    register_console_handler();
    let (hwnd_sender, hwnd_receiver) = sync_channel(1);
    let class_name: Vec<u16> = format!("{CLASS_PREFIX}_{}_{registration}\0", std::process::id())
        .encode_utf16()
        .collect();

    let thread = thread::Builder::new()
        .name("ghostlight-session-listener".into())
        .spawn(move || {
            // SAFETY: GetModuleHandleW with a null name returns this module's instance.
            let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let class = WndClassW {
                style: 0,
                lpfn_wnd_proc: window_proc,
                cb_cls_extra: 0,
                cb_wnd_extra: 0,
                h_instance: instance,
                h_icon: std::ptr::null_mut(),
                h_cursor: std::ptr::null_mut(),
                hbr_background: std::ptr::null_mut(),
                lpsz_menu_name: std::ptr::null(),
                lpsz_class_name: class_name.as_ptr(),
            };

            // SAFETY: RegisterClassW registers a class whose procedure is the static window_proc.
            let atom = unsafe { RegisterClassW(&class) };
            if atom == 0 {
                let _ = hwnd_sender.send(Err(io::Error::last_os_error()));
                return;
            }

            // A top-level window (no parent) with WS_POPUP and WS_EX_TOOLWINDOW receives the
            // session broadcasts, and without WS_VISIBLE it never shows on screen or in the
            // taskbar.
            // SAFETY: the arguments describe a valid hidden top-level window of the class above.
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW,
                    class_name.as_ptr(),
                    class_name.as_ptr(),
                    WS_POPUP,
                    0,
                    0,
                    0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null_mut(),
                )
            };
            if hwnd.is_null() {
                let error = io::Error::last_os_error();
                // SAFETY: UnregisterClassW removes the class registered above.
                unsafe {
                    UnregisterClassW(class_name.as_ptr(), instance);
                }
                let _ = hwnd_sender.send(Err(error));
                return;
            }

            let _ = hwnd_sender.send(Ok(hwnd as isize));

            let mut msg = Msg::default();
            // SAFETY: GetMessageW retrieves this thread's messages until WM_QUIT.
            while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
                // SAFETY: standard Win32 message dispatch.
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            // SAFETY: the message loop has ended, so the class has no window left.
            unsafe {
                UnregisterClassW(class_name.as_ptr(), instance);
            }
        })?;

    let hwnd = hwnd_receiver
        .recv()
        .map_err(|_| io::Error::other("the session listener thread ended before it started"))??;
    Ok(Attached {
        hwnd: hwnd as Hwnd,
        thread: Some(thread),
    })
}
