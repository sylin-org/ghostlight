# ADR-0106: A session is the caller, not the connection

- Status: Accepted
- Date: 2026-08-11
- Supersedes the deferral in [ADR-0105](0105-scripted-intake-channels.md) Decision 1
- Builds on: ADR-0013, ADR-0096, ADR-0102, ADR-0104, ADR-0105

## Context

ADR-0105 shipped `ghostlight call` with workspaces admitted per connection, and deferred resumable
sessions on the grounds that a workspace outliving its connection needs a lifetime policy nobody
wants to invent: idle timeouts, expiry sweeps, and a way to reap a session whose owner crashed.

Living with it showed the deferral was worse than the problem. Typing two commands does not work:

```
> ghostlight call browser_open_page '{"url":"https://example.com"}'
Opened example.com.
> ghostlight call browser_read_page '{"tab":"tab_a1b2"}'
The requested workspace target is not currently usable.
```

And it is sharper than a lost handle. A session's tabs are closed when it ends, so a one-shot
mutation has nothing to carry forward to: the tab opens and the service closes it milliseconds
later, when the process exits. On a default install the extension's preserve-tabs interlock refuses
that close, so the tab survives owned by nobody, which is worse than either outcome on purpose.

`--stdin` works around it by holding one process open, which is why `scripts/browser-journey.ps1`
is built the way it is. That is a workaround for a model that does not match how people work.

## Decision

### 1. The session is the calling process

A session is keyed on its caller. Every `ghostlight call` from one terminal, or from one program
that shells out repeatedly, reaches the same workspace and therefore the same tabs.

The mental model is the one people already have: **the session is my terminal.** Nothing to name,
nothing to remember, and closing the terminal closes what it opened.

### 2. Identity is pid plus start time; the name is attribution

A process id alone is recycled. A process id with a name matches a recycled id whenever the
replacement runs the same program, which is the common case rather than the rare one: two
`pwsh.exe` in a row is the normal way a terminal is used. The pair the operating system keeps
unique is **pid and start time**, so that is identity.

The executable name travels with it for attribution, labels, and audit, and never for identity.
This is the same split already drawn between `client_label` and `channel` in ADR-0105.

### 3. `GHOSTLIGHT_SESSION` pins a session explicitly

A program that shells out *through a shell* gets a new parent on every call, because the parent is
a throwaway `cmd.exe` rather than the program. This is not hypothetical: Node's `exec` uses a shell
and `execFile` does not, Python's `subprocess(shell=True)` does, .NET's `Process.Start` with
`UseShellExecute=false` does not. An integrator would hit it and have no idea why their tabs kept
vanishing.

Environment is inherited through intermediaries, so an explicit key survives what the process tree
does not. When `GHOSTLIGHT_SESSION` is set it is the key; otherwise the caller is the parent.

The key is caller-supplied, so it is a claim rather than an observation. That is acceptable for the
same reason the channel is: a forged session key reaches the forger's own tabs, as themselves, on
their own machine. **It must never become an input to an authority decision**, exactly like the
channel.

### 4. Ownership decides lifetime, and liveness is observed rather than invented

A workspace with an owner survives its connection. It is released, and the tabs it holds are
closed, when its owner is gone.

This is the part that makes the decision affordable, and it is why the deferral in ADR-0105 no
longer applies. "When do I reap an abandoned session?" stops being a policy to invent and becomes a
fact to observe: is that process still running, and is it still the same process? An idle timeout
would have been a guess; this is an answer.

Consequences that follow:

- Reaping happens when a session is admitted, so its cost is proportional to use rather than to a
  timer, and a service with no callers does no work.
- A leased workspace is never reaped. Work in flight owns its handles until it completes.
- A declared key names no process and cannot be liveness-checked, so it lives until the service
  does. The workspace registry is in memory, so that bound is real rather than unbounded.
- A connection that sends no marker keeps the old behavior exactly. The MCP edge sends none, so
  nothing about a model's session changes.

### 5. Reading the caller costs a dependency, not an invariant

Parent pid, start time, and name come from `sysinfo` with default features off, which is safe Rust
over the platform APIs. The alternative was raw Win32 FFI against `unsafe_code = "forbid"`.

Unlike the signer gating deferred in ADR-0105, this is not a security control, so a forgeable value
is fit for purpose here: the worst outcome of a wrong answer is a caller that loses session
continuity, not one that gains authority.

## Amendment 2026-08-11 (releasing a session is not the same as closing its tabs)

Observed on the first live deployment. Decision 4 says an abandoned workspace is released "and the
tabs it holds are closed". The release is unconditional; the close is not.

The reaper closes tabs through the same browser command a model uses, so the extension's
preserve-tabs interlock applies to it, and that setting is on by default. On an ordinary install the
tabs of a finished session are therefore **released but not closed**: Ghostlight stops controlling
them, and they stay visible in the browser.

That is the correct outcome rather than a defect -- ADR-0060's interlock exists precisely so that a
local human setting outranks any decision the service makes, and keeping visual evidence is a
standing product directive. The error was in the wording. "Close your terminal and its tabs close"
should read: closing the terminal ends the session and hands its tabs back to you, and whether they
close is the browser's setting to decide.

## Consequences

- **Tabs now outlive the command.** `ghostlight call browser_open_page` leaves a real, owned tab
  until the terminal that opened it exits. This is the point of the change and it is a behavior
  change people must learn. "Close your terminal, your Ghostlight tabs close" is the rule, and it
  matches how a shell already owns its background jobs.
- `--stdin` remains useful for piping a fixed list of calls, but it is no longer the only way to do
  multi-step work.
- Tab groups stay keyed by channel, not by session. Grouping per session would give someone with
  four terminals open four `Ghostlight - ...` groups in their tab strip.
- A caller whose parent cannot be identified falls back to a connection-bound workspace. Losing
  continuity is a smaller failure than guessing an owner.

## Alternatives considered

- **An interactive `ghostlight shell`.** Rejected. It solves the typing case with one process and
  no domain change, but it asks people to enter a mode before working, and it does nothing for a
  program that shells out. The session marker serves both without a mode.
- **Named sessions (`--session work`).** Rejected. It invents identity where the operating system
  already supplies one, and it still needs the lifetime policy that made ADR-0105 defer the whole
  idea.
- **Idle timeouts.** Rejected as the primary rule. It is a guess about what a caller is doing, and
  it is wrong in both directions: it reaps a slow script and keeps a dead one.
- **Keying on pid plus name.** Rejected as identity for the recycling reason in Decision 2. Kept as
  attribution.
