# SPDX-License-Identifier: Apache-2.0 OR MIT
# Fish completion for ghostlight.
#
# The command and policy command lists below are checked against the command line's own lists,
# and the client list against the agent catalog, by tests in the orchestrator. Adding a
# subcommand, a policy command or an agent without adding it here fails those tests.

set -l ghostlight_commands open install uninstall doctor status call diagnostics policy update
set -l ghostlight_policy_commands validate migrate explain simulate keygen pubkey sign publish
set -l ghostlight_clients codex claude-code claude-desktop cursor vscode windsurf devin zed zcode opencode crush copilot-cli cline kiro qwen-code junie kilo-code goose continue antigravity cline-cli cline-vscode cline-cursor cline-windsurf

complete -c ghostlight -f

complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a open -d "Open the desktop workbench"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a install -d "Connect browsers and detected MCP clients"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a uninstall -d "Remove only Ghostlight-owned registrations"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a doctor -d "Check the complete local installation"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a status -d "Check the local service endpoint"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a update -d "Ask GitHub for a newer release"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a call -d "Run one browser tool"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a diagnostics -d "Read the shared diagnostics log, or turn it on and off"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a policy -d "Validate, explain, sign, or publish policy"

complete -c ghostlight -n "__fish_seen_subcommand_from install" -l dry-run -d "Show changes without writing them"
complete -c ghostlight -n "__fish_seen_subcommand_from install uninstall" -l browser -d "Select one supported browser" -x -a "chrome edge brave chromium"
complete -c ghostlight -n "__fish_seen_subcommand_from install" -l all-browsers -d "Select every supported browser"
complete -c ghostlight -n "__fish_seen_subcommand_from install uninstall" -l client -d "Select an agent, or one host of it" -x -a "$ghostlight_clients"
complete -c ghostlight -n "__fish_seen_subcommand_from install" -l all-clients -d "Include clients not currently detected"
complete -c ghostlight -n "__fish_seen_subcommand_from install" -l no-clients -d "Leave MCP client configuration unchanged"
complete -c ghostlight -n "__fish_seen_subcommand_from install" -l no-open -d "Do not open the Ghostlight window"
complete -c ghostlight -n "__fish_seen_subcommand_from uninstall" -l dry-run -d "Show changes without writing them"
complete -c ghostlight -n "__fish_seen_subcommand_from doctor" -l json -d "Print one JSON document"
complete -c ghostlight -n "__fish_seen_subcommand_from doctor" -l fix -d "Apply ownership-safe repairs"
complete -c ghostlight -n "__fish_seen_subcommand_from status" -l json -d "Print one JSON document"
complete -c ghostlight -n "__fish_seen_subcommand_from update" -l install -d "Install the newer release the check finds"
complete -c ghostlight -n "__fish_seen_subcommand_from call" -l json -d "Print the structured result"
complete -c ghostlight -n "__fish_seen_subcommand_from call" -l stdin -d "Run a batch over one session"
complete -c ghostlight -n "__fish_seen_subcommand_from call" -l output -d "Write bounded content to a file" -r -F
complete -c ghostlight -n "__fish_seen_subcommand_from call" -l catalog -d "List the available browser tools"
complete -c ghostlight -n "__fish_seen_subcommand_from call" -l help -s h -d "Show the usage of call"
complete -c ghostlight -n "__fish_seen_subcommand_from policy" -x -a "$ghostlight_policy_commands"
