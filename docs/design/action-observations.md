# Observing what an action did

The workbench says which tool ran, whether authority admitted it, and what happened without storing
page payloads. This note records the two structural owners that keep that account complete.

## The wrong cut

The obvious approach is to have each tool independently report a sentence and measurements. Every
terminal construction site in `crates/orchestrator/src/work/mod.rs` would decide both.

That makes every tool responsible for remembering two parallel accounts, which means the sentence
and machine projection can drift. It is the same failure the reduced-motion selector list had in
the extension: correctness maintained by memory rather than by structure.

## The two structural owners

There are two different facts, and each has one owner.

Every browser command in the executor goes through one seam:

```rust
fn dispatch(&self, context: &InvocationContext<'_>, command: BrowserCommand)
    -> Result<BrowserOutcome, BrowserError>
```

Twenty-five call sites funnel through it, and it is the only route to the browser port apart from
one compensating close. The seam records facts about the browser crossing:

- the landed host, never the path, query, or fragment; and
- the readiness the adapter reported.

That match remains exhaustive over `BrowserOutcome`. A new browser outcome does not compile until
someone decides whether it carries host or readiness.

Counts and sizes are not browser-generic facts. Their meaning comes from the product sentence:
three fields, seven matches, 1,240 words, or a 1280x720 capture. They belong to the required typed
`Outcome` in `language/outcome.rs`. `Outcome::summary()` and `Outcome::observed()` read the same
value, so a sentence and its measurement cannot diverge. `succeeded` cannot be called without an
`Outcome`.

`finish` reads and clears the seam observation, then merges the outcome observation over it. One
completion path produces the audit record. Neither guarantee depends on a future tool remembering
another reporting call.

## What may be observed

Measurements and metadata about the action. Never its content.

| Recorded | Not recorded |
| --- | --- |
| Landed **host** | Path, query, or fragment, where identifying detail lives |
| Elapsed time and reported readiness | Page text |
| Counts: fields filled, matches found, tabs listed, steps run | Field values, matched text |
| Screenshot dimensions and scope | Screenshot pixels |
| Result type and size | Result value |
| File count and total size | File names, paths, or contents |

The host is the deliberate line. It answers "where did the agent go" and is already visible in the
user's own tab strip, while the path is where a patient id or a record number would sit.

This is narrower than the model-facing `facts` on `InvocationResult`, which legitimately carries
page text and URLs. Those must never be copied into the audit record wholesale. The observation is
a separate, closed, typed value for exactly that reason.

## Shape

```rust
/// Content-free landing facts and outcome measurements about one action.
struct Observed {
    host: Option<String>,
    readiness: Option<String>,
    count: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
}
```

`count` is deliberately general and takes its meaning from the outcome summary beside it: "Filled
3 fields", "Found 7 matches", "Listed 4 controlled tabs". A field per tool would not survive the
catalog growing. The JSON shape did not change when the type moved from `governance` to `language`.

## How the surface uses it

The row already has columns for the tool, the elapsed time, the client, and the capability. It
renders the Ghostlight-authored sentence directly and adds only a readiness note when useful:

```
[nav]  browser_navigate    Opened example.com.             claude-code  action  2.5s  7h
[scan] browser_read        Read 1,240 words.                claude-code  read    0.4s  7h
[key]  browser_fill_form   Filled 3 fields.                 codex        write   1.1s  6h
[cam]  browser_screenshot  Captured the viewport at 1280x720.  codex     read    0.9s  6h
```

The hero also keeps the sentence and carries host, capability, status, and readiness as separate
metadata where available.

Readiness earns its place on the unhappy path. `2.5s` is reassurance; `8.0s, never settled` is the
row that explains why an agent looked stuck, and a bare number cannot say it.

## Current implementation

1. `dispatch` records host/readiness in an invocation-keyed map through an exhaustive outcome match.
2. `Outcome` renders the sentence, next steps, and the counts/sizes that sentence names.
3. `finish` merges outcome measurements over seam landing facts and clears the map.
4. The workbench renders the sentence without its former `measured()` host-or-summary guess.

`duration_ms` remains measured in `execute` rather than at the seam, because it covers the whole
invocation rather than one browser round trip.
