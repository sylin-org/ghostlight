# Latest result

## [0031] Both sessions' work integrated; Windows CI repaired

windows-claude rebased its commits onto linux-claude's [0030] fixes, ran the full gate on the
combined tree (fmt, workspace clippy, cargo test --workspace, guards, workbench, extension and
integrity checks, all green), deployed it in place on the owner's Windows machine and checked it
live through the window's watch view.

On dev from windows-claude since [0026]:

- t01: every test removes its temporary files, also when it fails or panics: a `Scratch` guard
  per crate (`crate::scratch::Scratch` in the service). New tests should hold one.
- t02: the service's idle-exit tests run on a clock the test moves (`lifetime::HandClock`), so a
  loaded runner cannot make the service leave early (CI 39079174806).
- t03: the icon endpoint serves each app icon as a finished tile on a dark or light ground the
  service picks per icon, or as cached when the icon brings its own plate.
- t04: a Microsoft Store app's own copy of an agent's settings is found on every look (Windows
  only; ADR-0197 amendment "Packaged clients seen from outside"). Live: the owner's Store Claude
  Desktop now reads "Found, not connected" with its packaged settings path.
- a63364d3: the retired-pages tests in `install/user_assets.rs` gate their symlink step to Unix
  and hold a scratch guard; this repairs Windows CI (39087428922).

For linux-claude: gate Unix-only calls in tests with `#[cfg(unix)]`; L09 should look at Linux
icons on their new tiles.
