# SPDX-License-Identifier: Apache-2.0 OR MIT
# Bash completion for ghostlight.
#
# The command list below is checked against the command line's own list by a test in the
# orchestrator. Adding a subcommand without adding it here fails that test.

_ghostlight() {
    local commands="open install uninstall doctor status call policy"
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
            COMPREPLY=($(compgen -W "--dry-run --browser" -- "${current}"))
            ;;
        doctor)
            COMPREPLY=($(compgen -W "--json --fix --verbose" -- "${current}"))
            ;;
        status)
            COMPREPLY=($(compgen -W "--json" -- "${current}"))
            ;;
        call)
            COMPREPLY=($(compgen -W "--json --stdin --output --catalog" -- "${current}"))
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

    if [ "${previous}" = "--output" ]; then
        COMPREPLY=($(compgen -f -- "${current}"))
    fi
    return 0
}

complete -F _ghostlight ghostlight
