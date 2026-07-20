# Greenfield first-success acceptance

Status: ready to run

## Purpose

This is the publication gate for a person who did not build Ghostlight. It tests the exact public
journey from an ordinary machine to one useful, read-only browser result. It is not a feature demo,
a maintainer-led installation session, or a substitute for the platform lifecycle recipes.

The cohort is five to ten informed participants across at least three MCP clients and both Windows
and Linux. macOS joins after a visible live-browser platform pass is available.

## Ground rules

- Give the participant only `https://sylin.org/ghostlight/install.md` and the first task below.
- State the extension path honestly. While store review is open, recruit only people who knowingly
  accept Developer mode and an unpacked release extension.
- Do not screen-share, type commands, supply private corrections, or explain around a defect.
- A participant may use their MCP agent to interpret the public guide.
- Record every maintainer intervention as a failed unaided attempt, even if the final result works.
- Never collect credentials, cookies, private page content, browser profiles, or raw MCP payloads.
- Use a safe public page or a purpose-built test identity. Do not use a participant's sensitive
  authenticated application for first-success evidence.

## First task

Use this exact request after installation:

> In my current browser, summarize the active page and tell me which tab you used. Do not click or
> change anything.

The task passes only when the result comes through Ghostlight from a visible managed browser tab.
A model answering from prior knowledge or another browser integration is not a pass.

## Minimum matrix

| Dimension | Required coverage |
| --- | --- |
| Participants | 5 to 10 people who did not author Ghostlight |
| Operating systems | Windows and Linux |
| MCP clients | At least three, including one terminal client and one graphical editor client |
| Extension path | Current public default; manual release path must be named pre-release |
| Release | One exact published Ghostlight version for the cohort |
| Browser | Supported Chromium 116 or later; record browser and version |

Do not count the same person reinstalling repeatedly as multiple participants. A clean virtual
machine is acceptable for install mechanics, but the browser must remain visible and in the same
ordinary user context as the MCP client and service.

## Run sequence

1. Record the evidence header before Ghostlight is installed.
2. Give the participant the canonical install URL and no additional setup advice.
3. Let the participant complete installation, extension setup, client restart, and doctor.
4. Ask the participant to run the first task exactly as written.
5. Ask them to explain, in their own words, which browser context Ghostlight used, what stayed
   local, and whether an account or subscription was required.
6. Record the last successful stage, elapsed time, all confusion, and every intervention.
7. Remove or redact any accidental sensitive material before retaining the record.

## Evidence header

```text
run_id:
date_utc:
participant_id:              # pseudonymous
ghostlight_version:
install_url_revision:
operating_system:
browser_and_version:
mcp_client_and_version:
extension_path:              # store or unpacked release
existing_ghostlight_state:   # must be none for greenfield
started_at:
finished_at:
```

## Result record

```text
installer_completed: yes/no
extension_connected: yes/no
client_registered: yes/no
doctor_green: yes/no
first_task_completed: yes/no
visible_managed_tab_confirmed: yes/no
maintainer_interventions: 0
undocumented_steps: 0
participant_boundary_summary:
confusion_or_failure:
public_fix_needed:
outcome: success/partial/blocked
```

Aggregate only counts, durations, client/platform combinations, and categorized friction. Do not
publish participant-level records without explicit consent.

## Acceptance threshold

Broad publication is ready when all of these are true:

- at least five participants completed the run;
- at least 80 percent reached the first task without maintainer intervention;
- Windows, Linux, and three MCP clients each have at least one unaided pass;
- no unresolved blocker appeared three times;
- successful participants accurately describe the visible, local, user-context boundary;
- the current default extension path has one clean install-to-doctor-to-first-task pass.

If the Chrome Web Store listing clears during the cohort, treat the store path as a new journey.
Do not combine its results with the unpacked-extension path until the store path has its own clean
acceptance pass.

## Failure handling

Convert a repeated problem into one public artifact: an installer fix, doctor diagnosis, guide
correction, or bounded Issue. Rerun the affected stage with a new participant after the fix. Do not
rewrite a failed record into a success, and do not lower the threshold to preserve a launch date.

The deeper packaged-product lifecycle remains in
[linux-live-lifecycle.md](linux-live-lifecycle.md). This cohort proves first success, not every
restart, upgrade, recovery, or uninstall behavior.

