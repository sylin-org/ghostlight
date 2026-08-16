# SPDX-License-Identifier: Apache-2.0 OR MIT
# Fish completion for ghostlight.
#
# The command list below is checked against the command line's own list by a test in the
# orchestrator. Adding a subcommand without adding it here fails that test.

set -l ghostlight_commands open install uninstall doctor status call policy

complete -c ghostlight -f

complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a open -d "Open the desktop workbench"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a install -d "Connect browsers and detected MCP clients"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a uninstall -d "Remove only Ghostlight-owned registrations"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a doctor -d "Check the complete local installation"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a status -d "Check the local service endpoint"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a call -d "Run one browser tool"
complete -c ghostlight -n "not __fish_seen_subcommand_from $ghostlight_commands" -a policy -d "Validate, explain, sign, or publish policy"

complete -c ghostlight -n "__fish_seen_subcommand_from install" -l dry-run -d "Show changes without writing them"
complete -c ghostlight -n "__fish_seen_subcommand_from install uninstall" -l browser -d "Select one supported browser" -x -a "chrome edge brave chromium"
complete -c ghostlight -n "__fish_seen_subcommand_from install" -l all-browsers -d "Select every supported browser"
complete -c ghostlight -n "__fish_seen_subcommand_from install" -l client -d "Select one MCP client" -x
complete -c ghostlight -n "__fish_seen_subcommand_from install" -l all-clients -d "Include clients not currently detected"
complete -c ghostlight -n "__fish_seen_subcommand_from install" -l no-clients -d "Leave MCP client configuration unchanged"
complete -c ghostlight -n "__fish_seen_subcommand_from install" -l no-open -d "Do not open the extension walkthrough"
complete -c ghostlight -n "__fish_seen_subcommand_from uninstall" -l dry-run -d "Show changes without writing them"
complete -c ghostlight -n "__fish_seen_subcommand_from doctor" -l json -d "Print one JSON document"
complete -c ghostlight -n "__fish_seen_subcommand_from doctor" -l fix -d "Apply ownership-safe repairs"
complete -c ghostlight -n "__fish_seen_subcommand_from status" -l json -d "Print one JSON document"
complete -c ghostlight -n "__fish_seen_subcommand_from call" -l json -d "Print the structured result"
complete -c ghostlight -n "__fish_seen_subcommand_from call" -l stdin -d "Run a batch over one session"
complete -c ghostlight -n "__fish_seen_subcommand_from call" -l output -d "Write bounded content to a file" -r -F
complete -c ghostlight -n "__fish_seen_subcommand_from call" -l catalog -d "List the available browser tools"
complete -c ghostlight -n "__fish_seen_subcommand_from policy" -x -a "validate explain simulate keygen pubkey sign publish"
