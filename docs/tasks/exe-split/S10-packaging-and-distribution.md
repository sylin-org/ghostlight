# S10: Packaging + distribution sweep (three binaries)

Goal: every distribution surface ships and references the three executables. This is the one task
whose files were NOT fully pre-read at authoring: re-read EVERY file you edit, keep its existing
shape, and make the smallest edits that reach the pinned end state (SPEC section 11).

## STOP preconditions

- S9 not logged complete -> STOP.
- `.github/workflows/release.yml` has no step uploading RAW version-less binaries (search for the
  raw-binary upload the DISTRIBUTION memory describes) -> record a deviation and adapt: the
  archive part still applies; skip the raw-triple change and note it.

## Required changes (end-states; re-read each file first)

1. `.github/workflows/release.yml` (sanctioned exception):
   - build: `cargo build --release --locked --workspace --target <t>`;
   - Package step: copy all three bins into the archive dir (same names + platform suffix);
   - raw uploads: three raw files per target following the file's EXISTING single-bin naming
     pattern extended to `ghostlight-adapter-agent` / `ghostlight-adapter-browser`;
   - the `test` job: `cargo test --locked --no-fail-fast --workspace`.
2. `scripts/get.sh` + `scripts/get.ps1`: download the three raw binaries (same URL pattern x3)
   into the same directory; unix chmod each; still finish by running `ghostlight install`.
3. `packaging/npm/`: the launcher downloads the three raw binaries on first run; its bin script
   EXECS `ghostlight-adapter-agent` (argv passed through) -- the npm entry is what MCP clients
   launch. Keep stderr-only logging. Update its README section that names the binary.
4. `packaging/winget`, `packaging/scoop`, `packaging/homebrew` templates: extend file lists to
   the three binaries (textual; shapes preserved).
5. `docs/business/DISTRIBUTION.md`: one short subsection noting the three-binary artifact shape
   (plain style).
6. `README.md` + `CLAUDE.md`: surgical mentions only -- README's install section notes the three
   executables in one sentence; CLAUDE.md's architecture line gains the two adapter names. Do NOT
   restructure either document.

## Tests (pinned)

No cargo tests. Verification is textual:
- `grep -c "ghostlight-adapter-agent" .github/workflows/release.yml` prints a number >= 1, and
  the same for `ghostlight-adapter-browser`;
- `grep -rn "ghostlight-adapter-agent" scripts/get.sh scripts/get.ps1 packaging/npm | wc -l`
  prints a number >= 3;
- `node --check` on any edited .mjs/.js files;
- SPEC section 12 (the cargo gates still pass -- this task must not break the build).

## Out of scope

Actually publishing anything (npm/CWS/winget are founder actions). Site content. Version bumps.

## Commit

`chore(dist): ship the three role executables across release, install scripts, npm, and manager templates (S10)`
