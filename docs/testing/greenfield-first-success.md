# Ghostlight 1.0 greenfield first-success acceptance

Status: planned release gate

## Purpose

Prove that a person who did not build Ghostlight can install the signed 1.0 package, understand the
tray workbench, connect one supported MCP harness and the matching extension, and obtain one useful
visible read without maintainer intervention.

## Ground rules

- Use one exact signed candidate and matching store-extension version for the cohort.
- Give participants only the release installation guide and first task below.
- Do not screen-share, type commands, or explain around a defect.
- Record every maintainer intervention as a failed unaided attempt.
- Use a safe public page and collect no credentials, cookies, page content, profile data, prompts,
  or raw MCP traffic.

## First task

> Open https://example.com in a new Ghostlight tab, summarize the page, and tell me which tab you
> used. Do not click, type, submit, or change the page after it opens.

The result must come through Ghostlight from a visible controlled tab in a dedicated or reused
Ghostlight group.

## Minimum matrix

| Dimension | Required coverage |
| --- | --- |
| Participants | 5 to 10 non-authors |
| Operating systems | Windows, macOS, and Linux |
| MCP harnesses | At least three, including a terminal client and graphical editor client |
| Browser | At least two supported Chromium families, version 116 or later |
| Installation | Signed package, OS-native uninstall path, matching store extension |
| Existing state | Clean machine or user profile with no Ghostlight 1.0 installation |

## Run

1. Record the evidence header.
2. Let the participant install the package and open Ghostlight from the tray.
3. Let them find **Installations**, Check and Install their harness registration, and reconnect the
   harness.
4. Let them install the matching store extension.
5. Ask for the exact first task.
6. Ask them to explain which browser context was used, what stayed local, how to pause work, and
   where they would look for health or history.
7. Let them use the workbench to remove the harness registration, then uninstall the package and
   extension through the documented paths.

## Evidence

```text
run_id:
date_utc:
participant_id:
ghostlight_version:
package_signature_or_digest:
operating_system:
browser_and_version:
mcp_harness_and_version:
extension_version:
install_started_at:
first_success_at:
uninstall_completed_at:
```

```text
package_installed: yes/no
tray_and_workbench_found: yes/no
harness_checked: yes/no
harness_registered: yes/no
extension_connected: yes/no
first_task_completed: yes/no
visible_group_confirmed: yes/no
checkup_understood: yes/no
pause_path_understood: yes/no
harness_entry_removed: yes/no
package_uninstalled_cleanly: yes/no
maintainer_interventions: 0
undocumented_steps: 0
confusion_or_failure:
outcome: success/partial/blocked
```

## Threshold

Broad publication requires:

- at least five completed runs;
- at least 80 percent reaching first success without intervention;
- an unaided pass on every supported operating system;
- at least three harnesses and two browser families represented;
- no unresolved blocker repeated three times; and
- one clean install-to-first-task-to-uninstall pass per platform.

Source-build or unpacked-extension success does not count toward packaged greenfield acceptance.
