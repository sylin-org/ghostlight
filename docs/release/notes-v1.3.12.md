# Ghostlight 1.3.12

Ghostlight 1.3.12 removes the browser-page control overlay and automatic session pauses after
policy refusals. Browser messages and animations remain passive; runtime controls belong in the
Tauri workbench. It also includes the unpublished 1.3.11 service changes, so these notes cover the
complete update from service 1.3.10.

### Upgrade compatibility

Use the matching 1.3.12 service and Chromium adapter. Adapter protocol major 3 is a breaking
change: the 1.3.12 adapter cannot work with service 1.3.10, and the older 1.3.8 adapter cannot work
with service 1.3.12. Update both parts together.

Supported browsers are Chrome, Edge, Brave, and Chromium. Chromium 125 or newer is required.

### Browser readiness and form filling

- Verify installation of the exact SHA-256-bound page runtime before making the browser adapter
  available. Existing pages and embedded frames receive the same service-owned runtime; newly
  attached out-of-process frames receive it before browser work resumes.
- Derive advertised browser capabilities from the adapter's implemented handlers, so unavailable
  mechanisms fail before dispatch.
- Fill forms within one shared deadline: validate all fields, dispatch browser input, release the
  debugger lease, then check that the complete batch retained its values before optional explicit
  submission. The default form-fill budget is 30 seconds; an explicit timeout still applies.

### Changed

- Repeated policy refusals no longer pause a session or require a review/resume step. Each request
  follows configured policy, and later permitted work can proceed immediately.
- Credential fields return guidance for the current request. When the user has explicitly
  authorized entry, fill and typing accept `user_authorized_credentials: true`, including on the
  first call. Earlier explicit authorization is sufficient. This per-request acknowledgement does
  not override configured policy or human Pause/Stop.
- Credential-like names on file controls no longer prevent explicitly requested uploads or
  recording attachments.

### Fixed

- Remove the attention dialog and its buttons from browser pages, including obsolete roots left
  by an earlier page runtime. Feedback cannot take focus or intercept pointer input even when its
  stylesheet is unavailable.
- Carry authorized credential entry through form fill, targeted typing, and focused typing while
  retaining the normal browser checks and excluding values from receipts and diagnostics.
- Update `rustls` to 0.23.45 for RUSTSEC-2026-0285.

### Internal cleanup

- Remove the unused raw BiDi/CDP bridge and Chromium raw-command path. The typed Ghostlight
  browser mechanisms remain the execution boundary.
- Remove unacknowledged preload injection and runnable headless browser-test paths. Browser
  component journeys use visible Chromium windows.
- Remove retired browser-adapter packaging and release paths from the Chromium-only offering.
  Installation and removal clean up obsolete native-host registrations only when Ghostlight
  ownership is proven, preserving foreign registrations.
