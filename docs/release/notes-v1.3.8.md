# Ghostlight 1.3.8

Ghostlight 1.3.8 brings the service into version lockstep with Chrome adapter 1.3.8
and introduces the Hub-and-Spoke browser adapter architecture and Mozilla Firefox
integration (ADR-0179).

## Highlights

- Hub-and-Spoke browser adapter architecture: decoupled browser engine handling
  with typed platform negotiation (`ghostlight/gecko` and `ghostlight/chromium`)
  for dynamic runtime adapter dispatch (ADR-0179).
- Mozilla Firefox WebExtension integration: complete Gecko adapter shell (`extension-firefox/`)
  implementing Manifest V3, background event page, native messaging relay, tabs, scripting,
  capture, and window management.
- Multi-browser native messaging host registration: unified CLI installer and diagnostic
  coverage for Chrome, Edge, Brave, Chromium, and Mozilla Firefox (`ghostlight install`,
  `ghostlight check`, `ghostlight uninstall`).
- Desktop workbench multi-browser UI: status and indicator support for multiple connected
  browsers and engines.
- Modal prompt race resolution: eliminated CDP input timeouts when clicking targets that
  trigger synchronous modal dialogs (`window.prompt`, `window.alert`, `window.confirm`).
- Version lockstep: service release and Chrome Web Store extension aligned at v1.3.8
  for seamless visual identification.

## Compatibility

Compatible with Chrome adapter 1.1.4 through 1.3.8 and Firefox adapter 1.3.8.
Chrome adapter 1.3.8 is published in the Chrome Web Store at 100% distribution.

## Install

Use `npx -y ghostlight@1.3.8`, the Windows installer, Debian package, or portable
archives. Release assets include checksums and GitHub build-provenance attestations.
