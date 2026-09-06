# H6: Useful work with explicit policy exclusions

Status: Policy direction agreed, not implemented. The owner accepted document-specific
authority, the three exclusion-handling choices, separate human notice choices, and the starting
profile below. Detailed result, capture, and presentation recommendations remain open. The
[ledger](LEDGER.md) owns progress, and ADR-0151's September 6 amendments record the decisions.

## Policy chooses the response to exclusions

Partial-result handling is a policy choice. The earlier recommendation to return permitted
content describes one useful mode, not mandatory behavior for every person or organization.
Use a small closed choice rather than independent booleans that permit contradictory settings.
Agreed person-facing choices, not manifest keys:

| When policy excludes embedded content | Behavior |
| --- | --- |
| Use permitted content | Return useful permitted observations with accurate coverage. An unrelated excluded embed does not prevent an allowed target operation. |
| Require complete access for this operation | Refuse when policy excludes any required part of the requested scope. A whole-page read refuses; a read of a fully permitted target can still succeed. |
| Require complete access for this page | Refuse browser-content work on a page containing an embedded document denied for the relevant capability, including otherwise permitted targets. Policy explanation and recovery controls remain available. |

These choices govern the response to an enforced exclusion. Existing host/RAWX grants and
observe/enforce semantics still decide which access is permitted. For example, permitting Read
while denying Write can make embedded fields readable without permitting edits. Do not add an
exclusion-handling flag that silently grants content access prohibited by another effective rule.

Compose restrictive behavior through the existing immutable authority snapshot. A lower tier or
request can require narrower work; it cannot relax an organization's mandatory requirement. Show
the effective choice and deciding layer in the existing policy destination. All-open behavior
remains unchanged when no access is excluded.

Choose human notice intensity separately: on-demand details, notice when a task is affected, or
notice whenever content is excluded. These are agreed preferences that may be managed, not a
new admission check. A quiet display can keep structured result coverage. Every result must
describe its actual scope truthfully; none of these modes allows omitted content to be represented
as inspected. Repeated prose warnings are a presentation choice, not the definition of correctness.

Exact policy keys, detailed scoping, notice rendering, and composition details remain open. The
existing schema is strict; these illustrative labels are not accepted configuration today.

## Agreed starting profile

Use permitted content, with human notices when work is affected. The person or organization can
choose the other handling and notice modes through the applicable policy/preference layer.

For an enforced host denial, retain useful permitted work and make the boundary of the result
explicit. Use "content excluded by policy" or "read limited by policy". Avoid "partial trust":
an exclusion describes what Ghostlight could access, not whether the rest of a website is true,
benign, or free of hostile instructions.

An iframe that is already loaded may remain visible to the human. Excluding Ghostlight access
does not mean Ghostlight prevented its load or stopped the site's own network activity. Do not
remove the frame, reload the page, or change policy as an automatic response to an exclusion.

## Operation behavior under "Use permitted content"

| Requested work | Proposed behavior |
| --- | --- |
| Read or inspect the composed page | Return only permitted text or controls. Mark that policy excluded embedded content; do not return excluded labels, values, selectors, or content-derived descriptions. |
| Find a control with no permitted match | Say no match was found in inspected content. Do not claim that no match exists anywhere on the page. |
| Wait for text to be absent across the page | Do not claim absence while part of the requested search scope is excluded. Return the coverage limitation. A positive observation in permitted content can still establish presence. |
| Read or act on a clearly identified permitted target | Complete normally when the required scope is covered. An unrelated excluded embed does not make this target's operation incomplete. |
| Act on a target whose document is denied | Refuse before the effect, with the applicable policy reason. Do not substitute another target merely because it is allowed. |
| Fill several explicitly requested fields | Check known target eligibility before the first write. If a known required target is denied, refuse before partially filling the rest. Later runtime failures still preserve honest partial effects; this is not an atomicity promise. |
| Capture a screenshot or recording that includes excluded content | Enforce the same disclosure boundary. Omit restricted regions only with reliable coverage; otherwise refuse that capture and provide a permitted alternative where available. Text filtering alone is insufficient. |

Excluded regions must be excluded before page-content extraction. Browser-observed routing and
document identity are the evidence needed for policy; reading blocked content first and trying
to remove it from a final MCP result is not this design.

## What the client sees

A page-wide read can return useful content with a short authored qualification:

```text
Read the permitted page content. One embedded site was excluded by policy.
```

A search with no permitted match should say:

```text
No matching control was found in the inspected content. One embedded site was excluded by policy.
```

A directly denied action can say:

```text
This control is in an embedded site excluded by policy. No action was taken.
```

That final sentence is appropriate only when the effect was actually prevented. A previous
effect in a multi-step operation remains in its account.

Each affected result also needs machine-readable coverage information, even if a person-facing
notice has already been shown. Distinguish operation outcome from observation coverage, and
distinguish policy exclusions from size limits and unavailable/navigating documents. These are
semantic requirements, not a selected wire schema or new top-level status vocabulary. The current
`truncated` boolean cannot convey all of them, and `Effect::Partial` describes effects rather
than missing portions of a read.

Completeness is relative to the requested supported scope. An explicit target read can be complete
while a whole-page read is limited. If no permitted content can satisfy the request, return a
clear limitation instead of an empty result that looks complete. Increasing `max_chars` or
repeating the same request cannot remove a policy exclusion; recovery guidance should reflect that.

## What the person sees

For a profile that shows exclusions, prefer a small persistent indication in existing
page/workbench governance surfaces:

```text
1 embedded site excluded
```

On expansion, show the policy reason and, if approved as part of this design, the bounded governed
host. A useful explanation is "Visible in your browser; excluded from Ghostlight access by
policy." Host disclosure would deliberately amend the historical rule keeping embedded origins
out of results; it does not justify full URLs, frame ids, paths, page titles, or blocked content.

Avoid repeated modal warnings or one toast per iframe. Keep any shown indicator current as
document state changes. Notice preferences can reserve proactive explanation for an affected
task or leave it available on demand. A blocked result still explains the refusal. The human
can inspect the governing rule or complete a restricted step themselves.
An organization restriction must not acquire an automatic per-action override.

Human notices may be deduplicated for unchanged page state. Coverage evidence on relevant tool
results must remain present because clients can reconnect, compact context, or read a later result
without having seen the earlier notice. Mark known exclusions without certifying the permitted
content as trustworthy.

## Consistency and evidence

Use one orchestrator-owned coverage vocabulary and authored outcome projection across read,
inspect, find, waits, actions, and capture. The extension supplies current document/geometry facts
and enforces instructed physical scope without making policy decisions. Connectors remain generic.
Bind the decision to the document actually observed or targeted; a navigating frame cannot retain
authority merely because it reuses an earlier browser frame id.

Acceptance fixtures should prove:

1. A permitted parent and denied child return the parent's permitted content with an explicit
   exclusion and no child content or target handles crossing into results.
2. A denied child containing the only matching control produces a scoped no-match result rather
   than a false page-wide absence claim.
3. An unrelated permitted target remains usable without repeated interruption.
4. A target that navigates into a denied document refuses on current authority before its next
   effect; already-dispatched uncertainty remains honest.
5. Capture cannot disclose excluded regions through screenshots, recordings, or related image
   artifacts. Unprovable exclusion produces a truthful refusal.
6. Size truncation, policy exclusion, and unavailable documents remain distinguishable; all-open
   still reads the complete supported composed page.
7. The same permitted-parent/denied-child fixture follows each chosen handling mode. Complete
   operation scope still permits an unrelated allowed target; complete page scope refuses it.
   Lower tiers cannot weaken a mandatory requirement, and the effective view names its source.
8. Notice preferences change human interruption without changing access or result coverage.
   Read-permitted/Write-denied fixtures distinguish readable fields from forbidden edits.

The current code does not provide this proof. `httpFrameIds` drops frame URLs before collection,
semantic results merge frames, and reads primarily report content plus a truncation bit. This
work needs a typed, revision-negotiated browser evidence contract and deliberate amendments to
the frame-transparency decisions in ADR-0151/0152, not a warning string added after collection.

## Scope of stricter choices

The complete-page choice is deliberately stronger than complete access for one operation. It
refuses more work, including otherwise permitted targets. Neither choice establishes that
Ghostlight prevented an embed from loading, guarantees support for every browser document, or
makes multi-step effects atomic. Availability failures and size limits retain their own meanings.

Do not infer either stricter choice from an ordinary denied host. The precise coverage schema,
host disclosure, notice placement, and reliable capture exclusion remain open. The policy
choices and starting profile are agreed; neither that agreement nor the acceptance fixtures
above establishes an implemented guarantee.
