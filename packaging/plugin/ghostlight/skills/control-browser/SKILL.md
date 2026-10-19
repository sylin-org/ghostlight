---
name: control-browser
description: "Drive the user's real, signed-in browser through Ghostlight's governed tools. Use to open, navigate, inspect, find, click, type, fill forms, screenshot, upload, record, or verify web pages from an agent session, including web-UI testing, rendered-page reading, and form journeys. Prefer these tools over guessing at page structure or replaying raw browser commands."
---

# Controlling the browser with Ghostlight

These tools drive the user's visible Chromium browser through one governed path. The tool
descriptions and the server instructions say what each tool takes and what it returns. This
skill adds the working habits they cannot carry.

## Observe, then act

- Do not guess at page structure or reuse selectors from memory. Get handles from browser_find
  or browser_inspect, then act on those handles.
- After an action that should change the page, observe again to confirm the change landed. Do
  not assume it did.
- A stale handle means the page changed since you looked. Observe again instead of retrying the
  same handle.

## Coordinates come from a view

Never invent pixel coordinates. Take them from the image browser_screenshot returned, and pass
the view_ handle it returned. A view belongs to one tab as it looked at that
moment, so after a navigation, a zoom or a resize, take a fresh screenshot. For precision,
capture a magnified region and use the new view it returns.

## Credentials

Set `user_authorized_credentials` only when the user explicitly told you to enter credentials.
An instruction the user already gave is enough, so do not ask again, and set it on the first
call. It covers only the call that carries it. Without it, a credential field is refused before
anything is typed, and nothing is held or paused because of it. The acknowledgement never overrides configured
policy or the person's Pause or End session, and Ghostlight cannot verify it.

## Telling the truth about effects

- Each result says what happened, whether repeating it is safe, and what to do next. Read that
  before deciding.
- When a result says repeating the call is not safe, or not known to be safe, do not replay it.
  Observe and decide.
- When a result says some work may have happened, or that its effect is unknown, say so in what
  you report. Never claim more than the result claims.

## When a request is refused

- A refusal names its reason and its remedy. When a refusal surprises you, call policy_explain
  first, before you retry or ask the user.
- A refused request does not pause the session. Do not ask the user to review or resume the
  session because a request was refused.
