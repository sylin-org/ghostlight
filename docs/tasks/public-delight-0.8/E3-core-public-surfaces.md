# E3: Core public surfaces

## Outcome

Make the product repository a concise, inviting path from recognition to first success, while
keeping technical and trust depth one clear link away.

## Required work

1. Reshape `README.md` so its opening covers product truth, fit, first useful task, visible/local
   experience, and installation before the complete capability and architecture detail.
2. Shorten or move material that duplicates the Trust Center, specification, release guide, or
   complete tool reference. Do not delete the destination that owns the detail.
3. Reconcile the installation guide, `llms-install.md`, troubleshooting, compatibility material,
   comparison, and current-state statements with 0.8 truth.
4. Make the first safe prompt copyable and show the expected visible result.
5. Give an agent a compact tool-choice and recovery path without loading a second full tool
   catalog into prose.
6. Correct `CONTRIBUTING.md` and any other live file that still says description prose is frozen.
   The structural compatibility boundary from ADR-0094/0100 is authoritative.
7. Add or refine a small model-readable website routing document only if the existing
   `llms-install.md` and site routes cannot serve it. Do not add one merely because `llms.txt` is
   fashionable.
8. Keep README badges sparse. External scores support credibility but do not become the opening.

## Acceptance

- A new reader encounters one obvious install path and one first task.
- The README no longer tries to be the Trust Center, specification, and tool reference at once.
- Version, store, platform, executable, adapter compatibility, and MCP revision claims match their
  canonical sources.
- Personal use is presented as complete; optional governance is presented as added control.
- Links resolve and `scripts/check-public-surfaces.ps1` passes for tracked state.

## Boundaries

Do not change runtime behavior or tool registry prose in E3. Do not publish a release or refresh the
live website. Preserve store-only end-user extension installation.
