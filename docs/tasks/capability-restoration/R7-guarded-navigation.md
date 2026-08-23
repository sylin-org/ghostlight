# R7: Guarded navigation

## Goal

Restore explicit unsaved-change discard with a name and boundary that do not imply a general force
bypass.

## Required work

- Add `beforeunload` to `browser_navigate`, defaulting to `stop`, with `discard` as the only other
  value.
- Default navigation reports a blocking beforeunload without accepting it.
- The discard branch accepts only a beforeunload produced by that navigation attempt, then
  continues through the ordinary commit and landing-governance path.
- Never accept an unrelated alert, confirm, or prompt. Preserve `browser_dialog` for those.
- Negotiate the guarded-navigation mechanism revision before dispatch.

## Evidence

- Schema and decoder default, valid, invalid, and shortest-call tests.
- Extension tests distinguish beforeunload from alert, confirm, and prompt.
- Executor tests prove default no-effect stop, explicit discard, redirect governance, cancellation,
  and uncertain dispatch.
- Real process and live-browser fixture journey with a dirty form.

## STOP conditions

- Chromium cannot correlate the accepted beforeunload with the requested navigation.
- The implementation must auto-accept any dialog type or weaken ordinary dialog handling.
- The discard choice could bypass destination authority or committed-landing governance.

## Commit

`feat(browser): restore guarded navigation`

