# Ghostlight -- Claude Code instructions

The canonical, model-agnostic agent guide for this repository is [AGENTS.md](AGENTS.md).
Everything that used to live in this file (project identity, architecture, the sacred
tool-schema constraint, code style, test strategy, scope exclusions) moved there so that
every agentic tool reads the same instructions. This file just imports it:

@AGENTS.md

Claude-specific note: your auto-memory for this project is a cache, not the source of
truth. Decisions live in `docs/adr/`, current state in `docs/STATUS.md`, machine-local
facts in `local/MACHINE-STATE.md`. When memory and the tree disagree, the tree wins.
