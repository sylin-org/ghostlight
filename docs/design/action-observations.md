# Observing what an action did

The workbench can say which tool ran and whether authority admitted it. It cannot yet say what
happened: how long a page took to settle, how many fields were filled, how large a capture was.
This note records where that observation belongs, so the next person does not thread it through
fifty call sites by hand.

## The wrong cut

The obvious approach is to have each tool report its own facts. Every terminal outcome carries
them upward, `Terminal` grows a field, and each of the fifty construction sites in
`crates/orchestrator/src/work/mod.rs` decides what to put in it.

That makes every tool responsible for remembering, which means a new tool is silent by default and
the surface degrades one addition at a time. It is the same failure the reduced-motion selector
list had in the extension: correctness maintained by memory rather than by structure.

## The right cut

An observation is a fact about crossing the browser boundary, not about a use case.

Every browser command in the executor goes through one seam:

```rust
fn dispatch(&self, context: &InvocationContext<'_>, command: BrowserCommand)
    -> Result<BrowserOutcome, BrowserError>
```

Twenty-five call sites funnel through it, and it is the only route to the browser port apart from
one compensating close. That seam already knows everything worth recording:

- when the command went out and when the outcome came back, which is the settle time;
- the readiness the adapter reported;
- the outcome itself, which carries the committed landing and any counts.

So `dispatch` observes, keyed by invocation, and `finish` reads the accumulated observation when
it writes the audit record. One place. Every tool benefits, including tools not written yet.

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
/// Content-free observations about one action, gathered at the browser boundary.
struct Observed {
    host: Option<String>,
    readiness: Option<String>,
    count: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
}
```

`count` is deliberately general and takes its meaning from the summary beside it: "Filled 3
fields", "Found 7 matches", "Listed 4 controlled tabs". A field per tool would not survive the
catalog growing.

## How the surface uses it

The row already has columns for the tool, the elapsed time, the client, and the capability. An
observation fills the description column and nothing else, so nothing is said twice:

```
[nav]  browser_open_page   example.com          claude-code  read   2.5s  7h
[scan] browser_read_page   1,240 words          claude-code  read   0.4s  7h
[key]  browser_fill_form   3 fields             codex        write  1.1s  6h
[cam]  browser_take_screenshot  viewport, 1280x720  codex     read   0.9s  6h
```

The hero has room for the sentence: "Opened example.com. Settled in 2.5s."

Readiness earns its place on the unhappy path. `2.5s` is reassurance; `8.0s, never settled` is the
row that explains why an agent looked stuck, and a bare number cannot say it.

## Order of work

1. `Observed` and the map on the executor, recorded in `dispatch`, read in `finish`.
2. Host and readiness, which are already returned and currently discarded.
3. Counts, tool by tool: `read_page`, `fill_form`, `find`, `take_screenshot`, `wait`.

Each step lands on its own and improves its own rows. `duration_ms` already ships and is measured
in `execute` rather than at the seam, because it covers the whole invocation rather than one
browser round trip; the two are different spans and both are worth having.

## An adjacent wording fix

`docs/1.0/INTENT.md` says file upload "never records paths, names, or bytes in audit." Read as
file contents that is correct and a byte count is fine, but the sentence should say "or contents"
before any size is recorded, so the document and the code cannot be read as disagreeing.
