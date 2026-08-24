# Latest result

## [0017] Lane brief -- Linux verification of the foundry sprint (windows-codex to linux-codex)

Authority: verify and, if needed, fix with regression coverage. No main merge, no tag, no
publish/store/release action, no network behavior added.

Lane head: origin/dev at `8f400eaf` or later on dev.

Full self-contained instructions:
`docs/tasks/demo-press-key-diagnosis/START-HERE-LINUX.md` -- follow top to bottom.

Summary of what must happen:

1. Sync to the lane head; record the exact HEAD sha in your evidence.
2. Source gates: fmt, warnings-denied Clippy, full Rust workspace tests, extension npm tests.
3. Rebuild and deploy your established user-level candidate from this exact revision
   (orchestrator plus both connectors), then explicitly reload the unpacked extension at
   chrome://extensions. The reload is mandatory: the content script changed, and skipping it
   reproduces the old defect.
4. Run `scripts/demo-foundry.sh` end to end against the ordinary visible Chromium profile.
   Required: every beat green, including mid-story `key to end` and the desk-stage
   ring/status/answer/dismiss sequence.
5. Honest-rendering spot checks: one deliberate primitive refusal shows "The browser refused
   this job: ..." with facts `browser_primitive_failed` + detail (never a disconnection
   sentence); one failing audit record carries `refusal_facts`; success records do not.
6. Add a dated CachyOS record under docs/testing/, link it from the diagnosis ledger, update
   STATUS.md's in-flight paragraph if stale, commit logical changes separately, push dev,
   report back via CHAT.md.

Context: this proves the same-day Windows sprint that fixed the foundry press_key failure
and the desk-bell blocking-dialog hang (activation now replies before dispatching). Full
mechanisms: `docs/tasks/demo-press-key-diagnosis/LEDGER.md`.

Known honest nuance, not a defect: with reply-before-dispatch, a fast dialog status right
after a click may truthfully see no dialog yet. Beats still pass.
