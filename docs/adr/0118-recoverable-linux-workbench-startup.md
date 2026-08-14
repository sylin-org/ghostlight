# ADR-0118: Recoverable Linux workbench startup

- Status: Accepted
- Date: 2026-08-13
- Amends: ADR-0112 Decision 1
- Builds on: ADR-0116

## Context

ADR-0112 requires one normal desktop startup with a tray and a workbench that does not interrupt
the user. It chose an initially minimized window on every platform.

The GTK3 window used by Tauri cannot reliably recover that initial minimized state under KDE
Wayland. Tauri also queues show and deiconify through GLib, while its focus guard immediately reads
the prior minimized and visible state. A restore request can therefore return successfully while
the compositor leaves the workbench minimized or drops the focus request.

An initially hidden GTK window avoids the unrecoverable compositor state. It preserves the product
promise: the tray is visible, startup does not steal focus, and the same workbench can be mapped on
demand.

## Decision

### 1. Backgrounding is platform-specific

Windows keeps the ADR-0112 minimized startup. Linux creates the same complete desktop authority and
tray but leaves its configured workbench hidden until activation. This is not a second launch mode;
connectors still launch the same executable with no arguments, and `--headless` remains the only
presentation-free mode.

### 2. Linux reveal follows GLib event ordering

Linux reveal first queues show and deiconify through Tauri. It schedules focus at GLib idle priority,
after the normal-priority window changes have landed and Tauri's focus guard can observe the current
state. Tray clicks and authenticated second-instance activation keep using this one presentation
seam.

Windows reveal continues to use Tauri's ordinary deiconify, show, and focus operations.

## Consequences

- Linux startup has a visible tray but no taskbar window until the workbench is opened.
- Windows startup behavior is unchanged.
- The implementation has one small operating-system boundary around native presentation and no
  retry thread, timing delay, additional process, or alternate activation path.

## Evidence

- On KDE Wayland, the original startup remained compositor-minimized after both Tauri and GTK
  restore attempts.
- With hidden Linux startup and ordered focus, a second authenticated launch produced one
  compositor window named `Ghostlight` with `active=true`, `minimized=false`, and `hidden=false`.
