# Ghostlight 1.3.12

Ghostlight 1.3.12 removes the browser-page control overlay and automatic session pauses after
policy refusals. Browser messages and animations remain passive; runtime controls belong in the
Tauri workbench.

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

Use the matching 1.3.12 service and Chromium adapter. The adapter uses protocol major 3 and is not
compatible with the public 1.3.10 service. Chrome publication must remain staged until the matching
service is publicly available.
