#Requires -Version 7
<#
.SYNOPSIS
    Cut a Ghostlight release end to end: tag -> watch CI -> verify assets -> fill
    package-manager checksums -> update the homebrew tap -> publish npm -> report.

.DESCRIPTION
    One command for the whole release ritual, so the sequence lives in a script and not
    in a person's memory. Every step detects whether it is already done and skips it, so
    the script is safe to re-run after a partial failure (resume by re-invoking, or jump
    with -From).

    Releases are cut from `main`. The annotated tag `v<version>` is the ONLY trigger for
    the Release workflow (.github/workflows/release.yml); everything after the tag reacts
    to the assets that workflow produces.

    ORDERING INVARIANT (the 0.4.1 lesson): the npm launcher's version drives the release
    asset URL it downloads, so `npm publish` MUST come after the GitHub release assets
    exist. This script enforces that by construction -- npm is the last mutating step and
    verifies the assets first.

.PARAMETER Version
    The release version WITHOUT the leading v, e.g. 0.5.1. The tag becomes v0.5.1.

.PARAMETER DryRun
    Read and report only. No tag, no push, no file edits, no npm publish. Shows exactly
    what each step would do.

.PARAMETER Yes
    Skip the interactive confirmations on the two irreversible actions (tag push and npm
    publish). Required for non-interactive / automated runs.

.PARAMETER From
    Resume at a named step instead of the beginning. One of:
    preflight, tag, watch, verify, sums, tap, npm, report.

.PARAMETER SkipTap
    Do not touch the sylin-org/homebrew-tap repository.

.PARAMETER SkipNpm
    Do not publish to npm.

.EXAMPLE
    pwsh -File scripts/release.ps1 0.5.1
        Full release, with confirmations.

.EXAMPLE
    pwsh -File scripts/release.ps1 0.5.1 -DryRun
        Show the whole plan and validate preflight without changing anything.

.EXAMPLE
    pwsh -File scripts/release.ps1 0.5.1 -From sums
        Resume after a green release: fill sums, update the tap, publish npm.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory, Position = 0)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string] $Version,

    [switch] $DryRun,
    [switch] $Yes,

    [ValidateSet('preflight', 'tag', 'watch', 'verify', 'sums', 'tap', 'npm', 'report')]
    [string] $From = 'preflight',

    [switch] $SkipTap,
    [switch] $SkipNpm
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# --- Constants -------------------------------------------------------------------------

$RepoSlug = 'sylin-org/ghostlight'
$TapSlug = 'sylin-org/homebrew-tap'
$Tag = "v$Version"
$RepoRoot = Split-Path -Parent $PSScriptRoot

# The cross-build matrix (release.yml build job). Each archive is
# ghostlight-<Tag>-<target>.<ext>; each ships two raw versionless bins too.
$Targets = @(
    @{ Target = 'x86_64-pc-windows-msvc'; Ext = 'zip'; Windows = $true }
    @{ Target = 'aarch64-apple-darwin'; Ext = 'tar.gz'; Windows = $false }
    @{ Target = 'x86_64-apple-darwin'; Ext = 'tar.gz'; Windows = $false }
    @{ Target = 'x86_64-unknown-linux-gnu'; Ext = 'tar.gz'; Windows = $false }
)

$StepOrder = @('preflight', 'tag', 'watch', 'verify', 'sums', 'tap', 'npm', 'report')

# --- Output helpers --------------------------------------------------------------------

function Write-Banner([string] $Text) {
    Write-Host ''
    Write-Host "=== $Text ===" -ForegroundColor Cyan
}
function Write-Ok([string] $Text) { Write-Host "  [ok]   $Text" -ForegroundColor Green }
function Write-Skip([string] $Text) { Write-Host "  [skip] $Text" -ForegroundColor DarkGray }
function Write-Info([string] $Text) { Write-Host "  [info] $Text" }
function Write-Warn2([string] $Text) { Write-Host "  [warn] $Text" -ForegroundColor Yellow }
function Write-Would([string] $Text) { Write-Host "  [dry]  would: $Text" -ForegroundColor Magenta }

function Confirm-Or-Stop([string] $Prompt) {
    if ($Yes) { Write-Info "-Yes: proceeding without confirmation ($Prompt)"; return }
    $answer = Read-Host "$Prompt  Type 'yes' to proceed"
    if ($answer -ne 'yes') { throw "Aborted at confirmation: $Prompt" }
}

# Run a native command and throw on nonzero exit. Args passed as an array.
function Invoke-Native([string] $Exe, [string[]] $Arguments, [switch] $AllowFail) {
    Write-Info "$Exe $($Arguments -join ' ')"
    & $Exe @Arguments
    if ($LASTEXITCODE -ne 0 -and -not $AllowFail) {
        throw "$Exe exited $LASTEXITCODE"
    }
    return $LASTEXITCODE
}

# --- Asset naming ----------------------------------------------------------------------

function Get-ArchiveName([hashtable] $T) { "ghostlight-$Tag-$($T.Target).$($T.Ext)" }

function Get-ExpectedAssets {
    $names = [System.Collections.Generic.List[string]]::new()
    foreach ($t in $Targets) {
        $arc = Get-ArchiveName $t
        $names.Add($arc)
        $names.Add("$arc.sha256")
        $suffix = if ($t.Windows) { '.exe' } else { '' }
        foreach ($b in @('ghostlight', 'ghostlight-relay')) {
            $raw = "$b-$($t.Target)$suffix"
            $names.Add($raw)
            $names.Add("$raw.sha256")
        }
    }
    $ext = "ghostlight-extension-$Tag.zip"
    $names.Add($ext)
    $names.Add("$ext.sha256")
    return $names
}

# Download a .sha256 release asset and return the 64-hex digest it carries.
# The asset format is "<hash>  <filename>".
function Get-AssetSha256([string] $AssetName, [string] $TmpDir) {
    $pattern = "$AssetName.sha256"
    & gh release download $Tag --repo $RepoSlug --pattern $pattern --dir $TmpDir --clobber
    if ($LASTEXITCODE -ne 0) { throw "could not download $pattern from $Tag" }
    $file = Join-Path $TmpDir $pattern
    $line = (Get-Content -Raw $file).Trim()
    $hash = ($line -split '\s+')[0]
    if ($hash -notmatch '^[0-9a-fA-F]{64}$') { throw "unexpected sha256 content in ${pattern}: $line" }
    return $hash.ToLower()
}

# --- Version consistency ---------------------------------------------------------------

function Test-VersionConsistency {
    $problems = [System.Collections.Generic.List[string]]::new()

    function Check([string] $Path, [string] $Label, [string] $Pattern, [int] $MinCount = 1) {
        $full = Join-Path $RepoRoot $Path
        if (-not (Test-Path $full)) { $problems.Add("missing file: $Path"); return }
        $text = Get-Content -Raw $full
        $mm = @([regex]::Matches($text, $Pattern))
        $good = @($mm | Where-Object { $_.Groups['v'].Value -eq $Version }).Count
        $any = $mm.Count
        if ($good -lt $MinCount) {
            $seen = @($mm | ForEach-Object { $_.Groups['v'].Value } | Select-Object -Unique) -join ', '
            $problems.Add("$Label ($Path): expected $Version but found [$seen] ($any hit(s))")
        }
    }

    Check 'Cargo.toml' 'workspace crate' '(?m)^version = "(?<v>[^"]+)"'
    Check 'crates/core/Cargo.toml' 'core crate' '(?m)^version = "(?<v>[^"]+)"'
    Check 'crates/relay/Cargo.toml' 'relay crate' '(?m)^version = "(?<v>[^"]+)"'
    Check 'crates/transport/Cargo.toml' 'transport crate' '(?m)^version = "(?<v>[^"]+)"'
    Check 'extension/manifest.json' 'extension manifest' '"version":\s*"(?<v>[^"]+)"'
    Check 'packaging/npm/package.json' 'npm package' '"version":\s*"(?<v>[^"]+)"'
    Check 'server.json' 'server.json (2 fields)' '"version":\s*"(?<v>[^"]+)"' 2
    Check 'packaging/scoop/ghostlight.json' 'scoop version' '"version":\s*"(?<v>[^"]+)"'
    Check 'packaging/winget/Sylin.Ghostlight.yaml' 'winget PackageVersion' '(?m)^PackageVersion:\s*(?<v>\S+)' 3
    Check 'packaging/homebrew/ghostlight.rb' 'homebrew version' 'version "(?<v>[^"]+)"'

    return $problems
}

# --- sha256 edits (idempotent, position-based) -----------------------------------------

# Replace the string-valued "hash" in the scoop manifest (NOT the autoupdate object form).
function Set-ScoopHash([string] $Path, [string] $Hash) {
    $text = Get-Content -Raw $Path
    $new = [regex]::Replace($text, '("hash":\s*")[0-9a-fA-F]{64}|("hash":\s*")TODO[^"]*',
        { param($m) ($m.Groups[1].Value + $m.Groups[2].Value) + $Hash }, 1)
    if ($new -eq $text) { throw "scoop: no string hash placeholder matched in $Path" }
    Set-Content -Path $Path -Value $new -NoNewline
}

function Set-WingetHash([string] $Path, [string] $Hash) {
    $text = Get-Content -Raw $Path
    $new = [regex]::Replace($text, '(InstallerSha256:\s*)\S+', { param($m) $m.Groups[1].Value + $Hash }, 1)
    if ($new -eq $text) { throw "winget: no InstallerSha256 line matched in $Path" }
    Set-Content -Path $Path -Value $new -NoNewline
}

# In a homebrew formula, replace the sha256 that immediately follows the url line for a
# given target triple. Idempotent: matches any existing 64-hex or TODO token.
function Set-HomebrewShaForTarget([string] $Path, [string] $Target, [string] $Hash) {
    $text = Get-Content -Raw $Path
    $tq = [regex]::Escape($Target)
    $pattern = '(url "[^"]*' + $tq + '[^"]*"\s*\r?\n\s*sha256 ")([0-9a-fA-F]{64}|TODO[^"]*)(")'
    $new = [regex]::Replace($text, $pattern, { param($m) $m.Groups[1].Value + $Hash + $m.Groups[3].Value }, 1)
    if ($new -eq $text) { throw "homebrew: no sha256 line found after the $Target url in $Path" }
    Set-Content -Path $Path -Value $new -NoNewline
}

# =======================================================================================
# STEPS
# =======================================================================================

function Step-Preflight {
    Write-Banner 'Preflight'

    # Tooling present.
    foreach ($tool in @('git', 'gh')) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "required tool not found: $tool" }
    }
    if (-not $SkipNpm -and -not (Get-Command 'npm' -ErrorAction SilentlyContinue)) {
        throw "npm not found (use -SkipNpm to release without publishing)"
    }
    Invoke-Native 'gh' @('auth', 'status') | Out-Null
    Write-Ok 'git, gh, npm present; gh authenticated'

    Push-Location $RepoRoot
    try {
        # On main.
        $branch = (git rev-parse --abbrev-ref HEAD).Trim()
        if ($branch -ne 'main') {
            throw "releases are cut from main; you are on '$branch'. Run: git checkout main"
        }
        Write-Ok 'on branch main'

        # Clean tree (ignore untracked .claude/ scratch).
        $dirty = git status --porcelain=v1 | Where-Object { $_ -notmatch '^\?\? \.claude/' }
        if ($dirty) {
            Write-Warn2 'working tree not clean:'
            $dirty | ForEach-Object { Write-Host "         $_" }
            throw 'commit or stash changes before releasing'
        }
        Write-Ok 'working tree clean'

        # main == origin/main.
        Invoke-Native 'git' @('fetch', '--quiet', 'origin', 'main') | Out-Null
        $local = (git rev-parse main).Trim()
        $remote = (git rev-parse origin/main).Trim()
        if ($local -ne $remote) {
            throw "main ($($local.Substring(0,8))) != origin/main ($($remote.Substring(0,8))); push or pull first"
        }
        Write-Ok "main == origin/main @ $($local.Substring(0,8))"
    }
    finally { Pop-Location }

    # Version consistency.
    $problems = @(Test-VersionConsistency)
    if ($problems.Count -gt 0) {
        Write-Warn2 "version mismatches (expected $Version):"
        $problems | ForEach-Object { Write-Host "         $_" }
        throw 'fix version files before releasing'
    }
    Write-Ok "all version files agree on $Version"

    # CHANGELOG section.
    $changelog = Join-Path $RepoRoot 'CHANGELOG.md'
    $hasSection = Select-String -Path $changelog -Pattern "^## \[$([regex]::Escape($Version))\]" -Quiet
    if (-not $hasSection) { throw "CHANGELOG.md has no '## [$Version]' section" }
    Write-Ok "CHANGELOG has a [$Version] section"

    # Tag must not already exist (unless resuming).
    Push-Location $RepoRoot
    try {
        $existsLocal = (git tag --list $Tag)
        $existsRemote = (git ls-remote --tags origin $Tag)
        if ($existsLocal -or $existsRemote) {
            Write-Warn2 "$Tag already exists (local=$([bool]$existsLocal) remote=$([bool]$existsRemote)); the tag step will resume/skip"
        }
        else {
            Write-Ok "$Tag does not exist yet"
        }
    }
    finally { Pop-Location }
}

function Step-Tag {
    Write-Banner "Tag $Tag"
    Push-Location $RepoRoot
    try {
        $existsRemote = (git ls-remote --tags origin $Tag)
        if ($existsRemote) { Write-Skip "$Tag already on origin; not re-tagging"; return }

        $existsLocal = (git tag --list $Tag)
        if ($DryRun) {
            if (-not $existsLocal) { Write-Would "git tag -a $Tag -m 'Ghostlight $Tag'" }
            Write-Would "git push origin $Tag  (this fires the Release workflow)"
            return
        }

        Confirm-Or-Stop "About to create and PUSH tag $Tag from main (fires the release build)."
        if (-not $existsLocal) {
            Invoke-Native 'git' @('tag', '-a', $Tag, '-m', "Ghostlight $Tag") | Out-Null
            Write-Ok "created annotated tag $Tag"
        }
        else {
            Write-Skip "local tag $Tag already exists"
        }
        Invoke-Native 'git' @('push', 'origin', $Tag) | Out-Null
        Write-Ok "pushed $Tag"
    }
    finally { Pop-Location }
}

function Step-Watch {
    Write-Banner 'Watch the Release workflow'
    if ($DryRun) { Write-Would "gh run watch the Release run for $Tag (fail loudly on red)"; return }

    # If the release already exists with all assets, the run is done; skip.
    $existing = & gh release view $Tag --repo $RepoSlug --json name 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Skip "release $Tag already exists; not re-watching CI"
        return
    }

    # Find the run for this tag (headBranch == tag name for a tag push). Poll for it to register.
    $runId = $null
    for ($i = 0; $i -lt 12 -and -not $runId; $i++) {
        $json = & gh run list --repo $RepoSlug --workflow Release --branch $Tag --limit 1 `
            --json databaseId, status, headBranch 2>$null | ConvertFrom-Json
        if ($json -and $json.Count -gt 0) { $runId = $json[0].databaseId; break }
        Write-Info "waiting for the Release run to register ($($i + 1)/12)..."
        Start-Sleep -Seconds 10
    }
    if (-not $runId) { throw "no Release run found for $Tag after ~2 min; check GitHub Actions" }

    Write-Info "watching run $runId"
    & gh run watch $runId --repo $RepoSlug --exit-status
    if ($LASTEXITCODE -ne 0) { throw "Release workflow run $runId failed; see: gh run view $runId --repo $RepoSlug --log-failed" }
    Write-Ok "Release workflow succeeded (run $runId)"
}

function Step-Verify {
    Write-Banner 'Verify release assets'
    $expected = Get-ExpectedAssets

    if ($DryRun) {
        Write-Would "verify $Tag carries $($expected.Count) assets"
        $expected | ForEach-Object { Write-Info $_ }
        return
    }

    $view = & gh release view $Tag --repo $RepoSlug --json assets 2>$null | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or -not $view) { throw "release $Tag not found" }
    $actual = @($view.assets | ForEach-Object { $_.name })

    $missing = $expected | Where-Object { $_ -notin $actual }
    $extra = $actual | Where-Object { $_ -notin $expected }

    if ($missing) {
        Write-Warn2 "missing assets:"
        $missing | ForEach-Object { Write-Host "         $_" }
        throw "release $Tag is missing $($missing.Count) expected asset(s)"
    }
    Write-Ok "all $($expected.Count) expected assets present"
    if ($extra) {
        Write-Info "extra assets (not blocking):"
        $extra | ForEach-Object { Write-Host "         $_" }
    }
}

function Step-Sums {
    Write-Banner 'Fill package-manager checksums'
    $winTarget = 'x86_64-pc-windows-msvc'

    if ($DryRun) {
        Write-Would "download the 4 archive .sha256 assets from $Tag"
        Write-Would "set scoop hash + winget InstallerSha256 = sha256($winTarget zip)"
        Write-Would "set homebrew sha256 for aarch64-apple-darwin, x86_64-apple-darwin, x86_64-unknown-linux-gnu"
        Write-Would "commit packaging/{scoop,winget,homebrew} and push origin main"
        return
    }

    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) "ghostlight-sums-$Version"
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null

    # Compute hashes.
    $winZip = "ghostlight-$Tag-$winTarget.zip"
    $winHash = Get-AssetSha256 $winZip $tmp
    $armHash = Get-AssetSha256 "ghostlight-$Tag-aarch64-apple-darwin.tar.gz" $tmp
    $intelHash = Get-AssetSha256 "ghostlight-$Tag-x86_64-apple-darwin.tar.gz" $tmp
    $linuxHash = Get-AssetSha256 "ghostlight-$Tag-x86_64-unknown-linux-gnu.tar.gz" $tmp
    Write-Ok "fetched 4 archive checksums"

    $scoop = Join-Path $RepoRoot 'packaging/scoop/ghostlight.json'
    $winget = Join-Path $RepoRoot 'packaging/winget/Sylin.Ghostlight.yaml'
    $brew = Join-Path $RepoRoot 'packaging/homebrew/ghostlight.rb'

    Set-ScoopHash $scoop $winHash
    Set-WingetHash $winget $winHash
    Set-HomebrewShaForTarget $brew 'aarch64-apple-darwin' $armHash
    Set-HomebrewShaForTarget $brew 'x86_64-apple-darwin' $intelHash
    Set-HomebrewShaForTarget $brew 'x86_64-unknown-linux-gnu' $linuxHash
    Write-Ok "wrote checksums into scoop, winget, homebrew templates"

    Push-Location $RepoRoot
    try {
        $changed = git status --porcelain=v1 -- packaging/scoop packaging/winget packaging/homebrew
        if (-not $changed) {
            Write-Skip 'packaging checksums already up to date; nothing to commit'
            return
        }
        Invoke-Native 'git' @('add', 'packaging/scoop/ghostlight.json', 'packaging/winget/Sylin.Ghostlight.yaml', 'packaging/homebrew/ghostlight.rb') | Out-Null
        Invoke-Native 'git' @('commit', '-m', "chore(release): fill $Tag package-manager checksums") | Out-Null
        Write-Ok "committed checksum fill"
        Invoke-Native 'git' @('push', 'origin', 'main') -AllowFail | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Warn2 "push to origin/main was rejected (protected branch?). The commit is local; open a PR to land it."
        }
        else {
            Write-Ok "pushed checksum fill to origin/main"
        }
    }
    finally { Pop-Location }
}

function Step-Tap {
    Write-Banner 'Update the homebrew tap'
    if ($SkipTap) { Write-Skip '-SkipTap set'; return }

    if ($DryRun) {
        Write-Would "clone $TapSlug, set version+3 sums in Formula/ghostlight.rb, commit and push"
        return
    }

    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) "ghostlight-tap-$Version"
    if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
    Invoke-Native 'gh' @('repo', 'clone', $TapSlug, $tmp, '--', '--depth', '1') | Out-Null

    $formula = Join-Path $tmp 'Formula/ghostlight.rb'
    if (-not (Test-Path $formula)) { throw "tap formula not found at $formula" }

    # Recompute the same 3 sums (cheap; keeps this step independently runnable).
    $sumTmp = Join-Path $tmp '.sums'
    New-Item -ItemType Directory -Force -Path $sumTmp | Out-Null
    $armHash = Get-AssetSha256 "ghostlight-$Tag-aarch64-apple-darwin.tar.gz" $sumTmp
    $intelHash = Get-AssetSha256 "ghostlight-$Tag-x86_64-apple-darwin.tar.gz" $sumTmp
    $linuxHash = Get-AssetSha256 "ghostlight-$Tag-x86_64-unknown-linux-gnu.tar.gz" $sumTmp

    $text = Get-Content -Raw $formula
    # Bump version and normalize any hardcoded /vX/ url segment to this version.
    $text = [regex]::Replace($text, 'version "(?:[^"]*)"', "version `"$Version`"", 1)
    $text = [regex]::Replace($text, 'releases/download/v[^/]+/', "releases/download/$Tag/")
    Set-Content -Path $formula -Value $text -NoNewline
    Set-HomebrewShaForTarget $formula 'aarch64-apple-darwin' $armHash
    Set-HomebrewShaForTarget $formula 'x86_64-apple-darwin' $intelHash
    Set-HomebrewShaForTarget $formula 'x86_64-unknown-linux-gnu' $linuxHash
    Write-Ok 'updated tap formula (version + 3 sums)'

    Push-Location $tmp
    try {
        $changed = git status --porcelain=v1
        if (-not $changed) { Write-Skip 'tap already up to date'; return }
        Invoke-Native 'git' @('add', 'Formula/ghostlight.rb') | Out-Null
        Invoke-Native 'git' @('commit', '-m', "ghostlight $Version") | Out-Null
        Invoke-Native 'git' @('push') | Out-Null
        Write-Ok "pushed $Version to $TapSlug"
    }
    finally { Pop-Location }
}

function Step-Npm {
    Write-Banner 'Publish to npm'
    if ($SkipNpm) { Write-Skip '-SkipNpm set'; return }

    # Already published?
    $published = & npm view "ghostlight@$Version" version 2>$null
    if ($LASTEXITCODE -eq 0 -and $published) {
        Write-Skip "ghostlight@$Version already on npm; not re-publishing"
    }
    else {
        # Guard the ordering invariant: assets MUST exist before npm (the launcher fetches
        # them). Enforced for a live publish; a dry run only reports (the release may not
        # exist yet when previewing the whole plan up front).
        if (-not $DryRun) {
            & gh release view $Tag --repo $RepoSlug --json name 2>$null | Out-Null
            if ($LASTEXITCODE -ne 0) {
                throw "release $Tag does not exist; refusing to publish npm before its assets exist (the launcher would 404)"
            }
        }

        $npmDir = Join-Path $RepoRoot 'packaging/npm'
        if ($DryRun) {
            Write-Would "cd packaging/npm && npm publish --dry-run"
            Push-Location $npmDir
            try { & npm publish --dry-run } finally { Pop-Location }
        }
        else {
            Confirm-Or-Stop "About to PUBLISH ghostlight@$Version to the public npm registry."
            Push-Location $npmDir
            try { Invoke-Native 'npm' @('publish') | Out-Null } finally { Pop-Location }
            Write-Ok "published ghostlight@$Version"
        }
    }

    # Smoke: launcher fetches the version-matched binary and runs. doctor's exit code is
    # informational here (a machine with no browser/extension reports unhealthy by design).
    Write-Info "smoke test: npx -y ghostlight@$Version doctor"
    if ($DryRun) { Write-Would "npx -y ghostlight@$Version doctor"; return }
    & npx -y "ghostlight@$Version" doctor
    $code = $LASTEXITCODE
    if ($code -eq 0) { Write-Ok 'doctor healthy (exit 0)' }
    else { Write-Warn2 "doctor exited $code -- expected if this machine has no browser/extension set up; the launcher fetch itself worked if you saw output above" }
}

function Step-Report {
    Write-Banner 'Done -- manual remainder'
    Write-Host @"
  Automated by this script:
    - tag $Tag pushed, Release workflow watched to green
    - assets verified, package-manager checksums filled + committed
    - homebrew tap updated$(if ($SkipTap) { ' (SKIPPED)' })
    - npm publish + smoke$(if ($SkipNpm) { ' (SKIPPED)' })

  Still manual (by nature -- external systems / per-version submissions):
    - Chrome Web Store: resubmit ONLY if extension/ changed this release.
        (Rust-only releases reuse the pending review; see docs/legal/STORE_LISTING.md.)
    - Edge Add-ons: same zip as CWS, when the extension changed.
    - winget: a NEW PR per version to microsoft/winget-pkgs
        (copy the filled packaging/winget/Sylin.Ghostlight.yaml sections; needs the CLA).
    - MCP Registry: mcp-publisher with DNS auth on the sylin.org apex (founder-side).
    - Verify: https://github.com/$RepoSlug/releases/tag/$Tag
"@
}

# =======================================================================================
# DRIVER
# =======================================================================================

# Dot-sourcing (. ./release.ps1) loads the functions WITHOUT running the release, so the
# pure helpers can be unit-tested. Direct invocation runs the orchestration below.
if ($MyInvocation.InvocationName -eq '.') { return }

$startIndex = [array]::IndexOf($StepOrder, $From)
Write-Host "Ghostlight release $Tag" -ForegroundColor White
Write-Host "  mode: $(if ($DryRun) { 'DRY RUN (no mutations)' } else { 'LIVE' })  |  from: $From  |  skipTap: $SkipTap  |  skipNpm: $SkipNpm"

$dispatch = @{
    preflight = { Step-Preflight }
    tag       = { Step-Tag }
    watch     = { Step-Watch }
    verify    = { Step-Verify }
    sums      = { Step-Sums }
    tap       = { Step-Tap }
    npm       = { Step-Npm }
    report    = { Step-Report }
}

for ($i = $startIndex; $i -lt $StepOrder.Count; $i++) {
    & $dispatch[$StepOrder[$i]]
}

Write-Host ''
Write-Host "Release $Tag orchestration complete." -ForegroundColor Green
