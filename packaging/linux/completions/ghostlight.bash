# SPDX-License-Identifier: Apache-2.0 OR MIT
# Bash completion for ghostlight.
#
# The command list below is checked against the command line's own list, and the client list
# against the agent catalog, by tests in the orchestrator. Adding a subcommand or an agent
# without adding it here fails those tests.

_ghostlight() {
    local commands="open install uninstall doctor status call diagnostics policy"
    local clients="codex claude-code claude-desktop cursor vscode windsurf devin zed zcode opencode crush copilot-cli cline kiro qwen-code junie kilo-code goose continue antigravity cline-cli cline-vscode cline-cursor cline-windsurf"
    local current previous
    current="${COMP_WORDS[COMP_CWORD]}"
    previous="${COMP_WORDS[COMP_CWORD - 1]}"

    if [ "${COMP_CWORD}" -eq 1 ]; then
        COMPREPLY=($(compgen -W "${commands} --help --version" -- "${current}"))
        return 0
    fi

    case "${COMP_WORDS[1]}" in
        install)
            COMPREPLY=($(compgen -W "--dry-run --browser --all-browsers --client --all-clients --no-clients --no-open" -- "${current}"))
            ;;
        uninstall)
            COMPREPLY=($(compgen -W "--dry-run --browser --client" -- "${current}"))
            ;;
        doctor)
            COMPREPLY=($(compgen -W "--json --fix --verbose" -- "${current}"))
            ;;
        status)
            COMPREPLY=($(compgen -W "--json" -- "${current}"))
            ;;
        call)
            COMPREPLY=($(compgen -W "--json --stdin --output --catalog --help" -- "${current}"))
            ;;
        policy)
            if [ "${COMP_CWORD}" -eq 2 ]; then
                COMPREPLY=($(compgen -W "validate explain simulate keygen pubkey sign publish" -- "${current}"))
            else
                COMPREPLY=($(compgen -f -- "${current}"))
            fi
            ;;
        *)
            COMPREPLY=()
            ;;
    esac

    if [ "${previous}" = "--client" ]; then
        COMPREPLY=($(compgen -W "${clients}" -- "${current}"))
    fi
    if [ "${previous}" = "--output" ]; then
        COMPREPLY=($(compgen -f -- "${current}"))
    fi
    return 0
}

complete -F _ghostlight ghostlight
