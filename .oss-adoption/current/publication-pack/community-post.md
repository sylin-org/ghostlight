Status: complete
Run ID: 20260718T214944Z
Project: Ghostlight
Created: 2026-07-18T21:49:44Z

# Community post draft

Target community: Show HN after all launch predicates pass
Rules verified at (UTC): 2026-07-18
Rules: https://news.ycombinator.com/showhn.html

## Candidate title

> Show HN: Ghostlight - let any MCP client use your real logged-in browser

## Founder comment draft

I built Ghostlight because I wanted the MCP client I was already using to work in the Chromium
session where my actual work lives. I did not want another headless profile, a cloud browser, or a
browser feature tied to one model vendor.

Ghostlight is a local Rust service plus a thin Chromium extension. It gives Codex, Claude Code,
Cline, Cursor, OpenCode, and other stdio MCP clients the same browser surface. Work happens in a
managed tab group in the user's existing profile, with visible feedback for reading, typing,
clicking, screenshots, recording, and JavaScript. The person can see the scope, take the wheel, or
stop the session.

The model side mattered just as much as the visual side. The original trained browser schemas stay
byte-stable. Additive tools handle forms, files, multi-step scripts, waits, narration, recording,
and semantic actions with compact receipts, so useful work takes fewer calls and less context.

Everything in the runtime stays local. There is no Ghostlight account, activation server,
telemetry, or update ping. The browser automation core is Apache-2.0 OR MIT. There is a separately
licensed governance layer for organizations that want identity-bound domain/capability policy and
one local audit record per call; it never disables the software or phones home.

This is intentionally not a headless scraping or cloud-browser tool. It is for user-context work
where a person remains responsible for the authenticated browser. Governance constrains where and
what class of action may run; it does not pretend to infer semantic intent or solve in-domain
prompt injection.

Install and try the first read-only task:

```text
npx -y ghostlight install
```

Source, demo, architecture, limitations, and Trust Center are all in the repository:
https://github.com/sylin-org/ghostlight

I would especially value two kinds of feedback: whether the install reaches first useful work
without private help, and whether the visible boundary makes the browser feel understandable
without becoming distracting.

## Rule and tone check

- Personally built, non-trivial, and locally runnable.
- Direct try path with no Ghostlight account or email gate.
- Founder can remain present to discuss it.
- Technical explanation and limitations are in the post.
- No fundraiser, vote request, comment coordination, superlative, or disguised comparison attack.
- Publish only after store-backed first use is verified.

## Follow-up plan

Answer questions as the founder, not through generated comments. Record install failures,
misunderstandings, useful workflows, and support time. Correct public documentation quickly when
the correction is factual and safe. Use the normal development and review path for code changes.
Do not repost if the first submission is weak.

This draft is not authorization to submit it.
