//! Windows session shutdown detection through top-level broadcast window messages and console handler.
//!
//! When Windows shuts down or restarts, it broadcasts `WM_QUERYENDSESSION` and `WM_ENDSESSION`
//! to all top-level windows in the desktop session, and emits console events (`CTRL_SHUTDOWN_EVENT`,
//! `CTRL_LOGOFF_EVENT`) to console processes.
//!
//! Ghostlight's desktop authority must not intercept `RunEvent::ExitRequested` or prevent exit
//! when the system is terminating (ADR-0180). This module creates a hidden top-level window and
//! console control handler to detect system shutdown and signal the orchestrator to exit cleanly.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::ShutdownEvent;

type Hwnd = *mut core::ffi::c_void;
type Hinstance = *mut core::ffi::c_void;
type Hicon = *mut core::ffi::c_void;
type Hcursor = *mut core::ffi::c_void;
type Hbrush = *mut core::ffi::c_void;
type Hmenu = *mut core::ffi::c_void;
type Wparam = usize;
type Lparam = isize;
type Lresult = isize;

const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_QUERYENDSESSION: u32 = 0x0011;
const WM_ENDSESSION: u32 = 0x0016;

const WS_POPUP: u32 = 0x8000_0000;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;

const CTRL_LOGOFF_EVENT: u32 = 5;
const CTRL_SHUTDOWN_EVENT: u32 = 6;

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
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(module_name: *const u16) -> Hinstance;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
}

type SystemCallback = Arc<dyn Fn(ShutdownEvent) + Send + Sync + 'static>;

static NEXT_REGISTRATION_ID: AtomicUsize = AtomicUsize::new(1);
static SYSTEM_LISTENERS: Mutex<Vec<(usize, SystemCallback)>> = Mutex::new(Vec::new());

fn dispatch_system_event(event: ShutdownEvent) {
    let listeners = {
        let guard = SYSTEM_LISTENERS.lock().unwrap_or_else(|p| p.into_inner());
        guard.clone()
    };
    for (_, callback) in listeners {
        callback(event);
    }
}

unsafe extern "system" fn shutdown_window_proc(
    hwnd: Hwnd,
    msg: u32,
    w_param: Wparam,
    l_param: Lparam,
) -> Lresult {
    match msg {
        WM_QUERYENDSESSION => {
            dispatch_system_event(ShutdownEvent::Query);
            // Return 1 (TRUE) indicating the application allows shutdown to proceed.
            1
        }
        WM_ENDSESSION => {
            if w_param != 0 {
                dispatch_system_event(ShutdownEvent::Terminating);
            } else {
                dispatch_system_event(ShutdownEvent::Cancelled);
            }
            0
        }
        WM_CLOSE => {
            // SAFETY: DestroyWindow is called on the valid window owned by this message loop.
            let _ = DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            // SAFETY: PostQuitMessage posts WM_QUIT to this thread's message queue to exit GetMessageW.
            PostQuitMessage(0);
            0
        }
        // SAFETY: Delegating unhandled messages to DefWindowProcW with the original parameters.
        _ => DefWindowProcW(hwnd, msg, w_param, l_param),
    }
}

unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> i32 {
    if ctrl_type == CTRL_SHUTDOWN_EVENT || ctrl_type == CTRL_LOGOFF_EVENT {
        dispatch_system_event(ShutdownEvent::Terminating);
    }
    // Always continue to the default handler. Returning TRUE suppresses its ExitProcess call and
    // can leave a connector alive if its coordinated cleanup stalls during session teardown.
    0
}

fn ensure_console_ctrl_handler_registered() {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| {
        // SAFETY: SetConsoleCtrlHandler registers a valid system callback.
        unsafe {
            SetConsoleCtrlHandler(Some(console_ctrl_handler), 1);
        }
    });
}

/// RAII handle for an active system shutdown listener window.
pub struct SystemShutdownListener {
    hwnd: Hwnd,
    thread: Option<JoinHandle<()>>,
    registration_id: usize,
}

// SAFETY: Hwnd and JoinHandle can be sent between threads for shutdown cleanup.
unsafe impl Send for SystemShutdownListener {}
unsafe impl Sync for SystemShutdownListener {}

impl Drop for SystemShutdownListener {
    fn drop(&mut self) {
        {
            let mut listeners = SYSTEM_LISTENERS.lock().unwrap_or_else(|p| p.into_inner());
            listeners.retain(|(id, _)| *id != self.registration_id);
        }
        if !self.hwnd.is_null() {
            // SAFETY: PostMessageW posts WM_CLOSE to the dedicated listener window to trigger cleanup.
            unsafe {
                PostMessageW(self.hwnd, WM_CLOSE, 0, 0);
            }
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Start listening for Windows session shutdown messages.
///
/// Creates a hidden top-level window on a dedicated message-loop thread and registers a console
/// control fallback so every Ghostlight process promptly detects shutdown. The hidden window is
/// required even for console processes: Windows can classify a process that loads `user32.dll`
/// as a GUI application and omit `CTRL_LOGOFF_EVENT` and `CTRL_SHUTDOWN_EVENT`.
pub fn listen_for_system_shutdown(
    callback: Box<dyn Fn(ShutdownEvent) + Send + Sync + 'static>,
) -> io::Result<SystemShutdownListener> {
    ensure_console_ctrl_handler_registered();
    let registration_id = NEXT_REGISTRATION_ID.fetch_add(1, Ordering::SeqCst);
    {
        let mut listeners = SYSTEM_LISTENERS.lock().unwrap_or_else(|p| p.into_inner());
        listeners.push((registration_id, Arc::from(callback)));
    }

    let (hwnd_sender, hwnd_receiver) = sync_channel(1);
    let class_name_str = format!(
        "GhostlightShutdownListener_{}_{}\0",
        std::process::id(),
        registration_id
    );
    let class_name: Vec<u16> = class_name_str.encode_utf16().collect();

    let thread = thread::Builder::new()
        .name("ghostlight-win-shutdown".into())
        .spawn(move || {
            // SAFETY: GetModuleHandleW with null pointer retrieves current module instance.
            let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let class = WndClassW {
                style: 0,
                lpfn_wnd_proc: shutdown_window_proc,
                cb_cls_extra: 0,
                cb_wnd_extra: 0,
                h_instance: instance,
                h_icon: std::ptr::null_mut(),
                h_cursor: std::ptr::null_mut(),
                hbr_background: std::ptr::null_mut(),
                lpsz_menu_name: std::ptr::null(),
                lpsz_class_name: class_name.as_ptr(),
            };

            // SAFETY: RegisterClassW registers our window class with the static shutdown_window_proc.
            let atom = unsafe { RegisterClassW(&class) };
            if atom == 0 {
                let _ = hwnd_sender.send(Err(io::Error::last_os_error()));
                return;
            }

            // Top-level window (parent = NULL) with WS_POPUP and WS_EX_TOOLWINDOW:
            // Top-level status ensures Windows broadcasts WM_QUERYENDSESSION and WM_ENDSESSION to it.
            // Lack of WS_VISIBLE ensures it never displays on screen or in taskbar.
            // SAFETY: CreateWindowExW parameters create a valid hidden top-level window.
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
                // SAFETY: UnregisterClassW cleans up the registered window class on failure.
                unsafe {
                    UnregisterClassW(class_name.as_ptr(), instance);
                }
                let _ = hwnd_sender.send(Err(io::Error::last_os_error()));
                return;
            }

            let _ = hwnd_sender.send(Ok(hwnd as isize));

            let mut msg = Msg::default();
            // SAFETY: GetMessageW retrieves messages for this thread's windows until WM_QUIT.
            while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
                // SAFETY: Standard Win32 message dispatch.
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            // SAFETY: Clean up the window class once the message loop terminates.
            unsafe {
                UnregisterClassW(class_name.as_ptr(), instance);
            }
        })?;

    let hwnd_raw = hwnd_receiver
        .recv()
        .map_err(|_| io::Error::other("shutdown listener thread closed"))??;
    let hwnd = hwnd_raw as Hwnd;

    Ok(SystemShutdownListener {
        hwnd,
        thread: Some(thread),
        registration_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Duration;

    #[link(name = "user32")]
    extern "system" {
        fn SendMessageW(hwnd: Hwnd, msg: u32, w_param: Wparam, l_param: Lparam) -> Lresult;
    }

    #[test]
    fn shutdown_listener_receives_query_and_endsession_messages() {
        let query_received = Arc::new(AtomicBool::new(false));
        let terminating_received = Arc::new(AtomicBool::new(false));
        let cancelled_received = Arc::new(AtomicBool::new(false));

        let query_clone = Arc::clone(&query_received);
        let terminating_clone = Arc::clone(&terminating_received);
        let cancelled_clone = Arc::clone(&cancelled_received);

        let listener = listen_for_system_shutdown(Box::new(move |event| match event {
            ShutdownEvent::Query => query_clone.store(true, Ordering::SeqCst),
            ShutdownEvent::Terminating => terminating_clone.store(true, Ordering::SeqCst),
            ShutdownEvent::Cancelled => cancelled_clone.store(true, Ordering::SeqCst),
        }))
        .expect("shutdown listener starts");

        // Simulate Windows sending WM_QUERYENDSESSION
        // SAFETY: Sending message to our valid listener window handle.
        let result = unsafe { SendMessageW(listener.hwnd, WM_QUERYENDSESSION, 0, 0) };
        assert_eq!(result, 1, "WM_QUERYENDSESSION must return TRUE (1)");
        assert!(
            query_received.load(Ordering::SeqCst),
            "query event must be delivered"
        );

        // Simulate Windows sending WM_ENDSESSION (wParam = 0, cancelled)
        // SAFETY: Sending message to our valid listener window handle.
        let result = unsafe { SendMessageW(listener.hwnd, WM_ENDSESSION, 0, 0) };
        assert_eq!(result, 0, "WM_ENDSESSION must return 0");
        assert!(
            cancelled_received.load(Ordering::SeqCst),
            "cancelled event must be delivered"
        );

        // Simulate Windows sending WM_ENDSESSION (wParam = 1, terminating)
        // SAFETY: Sending message to our valid listener window handle.
        let result = unsafe { SendMessageW(listener.hwnd, WM_ENDSESSION, 1, 0) };
        assert_eq!(result, 0, "WM_ENDSESSION must return 0");
        assert!(
            terminating_received.load(Ordering::SeqCst),
            "terminating event must be delivered"
        );

        // Drop the listener handle and verify thread cleans up cleanly
        drop(listener);
        thread::sleep(Duration::from_millis(50));
    }

    #[test]
    fn console_shutdown_fallback_notifies_without_suppressing_default_exit() {
        let terminating_received = Arc::new(AtomicBool::new(false));
        let terminating_clone = Arc::clone(&terminating_received);
        let listener = listen_for_system_shutdown(Box::new(move |event| {
            if event == ShutdownEvent::Terminating {
                terminating_clone.store(true, Ordering::SeqCst);
            }
        }))
        .expect("shutdown listener starts");

        // SAFETY: This directly exercises our registered callback with a documented control code.
        let handled = unsafe { console_ctrl_handler(CTRL_SHUTDOWN_EVENT) };
        assert_eq!(
            handled, 0,
            "the default ExitProcess handler must remain enabled"
        );
        assert!(terminating_received.load(Ordering::SeqCst));
        drop(listener);
    }
}
