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

.PARAMETER SkipExtension
    Do not publish the browser extension (Chrome Web Store / Edge). The extension step is also
    skipped automatically when extension/ did not change since the previous tag.

.PARAMETER SkipWebsite
    Do not refresh the sylin.org website (the install-guide fallback + rebuild trigger).

.PARAMETER SkipRegistry
    Do not publish to the MCP registry. The registry step also self-skips when the
    MCP_DNS_PRIVATE_KEY env var (the DNS-auth key) is not set.

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

    [ValidateSet('preflight', 'tag', 'watch', 'verify', 'sums', 'tap', 'npm', 'registry', 'trust', 'extension', 'website', 'report')]
    [string] $From = 'preflight',

    [switch] $SkipTap,
    [switch] $SkipNpm,
    [switch] $SkipExtension,
    [switch] $SkipWebsite,
    [switch] $SkipRegistry
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# --- Constants -------------------------------------------------------------------------

$RepoSlug = 'sylin-org/ghostlight'
$TapSlug = 'sylin-org/homebrew-tap'
$Tag = "v$Version"
$RepoRoot = Split-Path -Parent $PSScriptRoot

# MCP registry publish (ADR/RELEASE.md): the pinned mcp-publisher release the registry step
# downloads on demand (bump deliberately), and the production registry it targets.
$McpPublisherVersion = 'v1.7.9'
$RegistryUrl = 'https://registry.modelcontextprotocol.io'

# The cross-build matrix (release.yml build job). Each archive is
# ghostlight-<Tag>-<target>.<ext>; each ships two raw versionless bins too.
$Targets = @(
    @{ Target = 'x86_64-pc-windows-msvc'; Ext = 'zip'; Windows = $true }
    @{ Target = 'aarch64-apple-darwin'; Ext = 'tar.gz'; Windows = $false }
    @{ Target = 'x86_64-apple-darwin'; Ext = 'tar.gz'; Windows = $false }
    @{ Target = 'x86_64-unknown-linux-gnu'; Ext = 'tar.gz'; Windows = $false }
)

$StepOrder = @('preflight', 'tag', 'watch', 'verify', 'sums', 'tap', 'npm', 'registry', 'trust', 'extension', 'website', 'report')

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
    & gh release view $Tag --repo $RepoSlug --json name 2>$null | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Skip "release $Tag already exists; not re-watching CI"
        return
    }

    # Find the run for this tag (headBranch == tag name for a tag push). Poll for it to
    # register (GitHub can take a minute+ to create the run after the tag push).
    # NOTE: pass a SINGLE --json field -- a comma-list with spaces would be parsed by
    # PowerShell as an array and split into separate args, silently breaking the query.
    $attempts = 18
    $runId = $null
    for ($i = 0; $i -lt $attempts -and -not $runId; $i++) {
        $json = & gh run list --repo $RepoSlug --workflow Release --branch $Tag --limit 1 --json databaseId 2>$null | ConvertFrom-Json
        if ($json -and @($json).Count -gt 0) { $runId = @($json)[0].databaseId; break }
        Write-Info "waiting for the Release run to register ($($i + 1)/$attempts)..."
        Start-Sleep -Seconds 10
    }
    if (-not $runId) { throw "no Release run found for $Tag after ~$([int]($attempts * 10 / 60)) min; check GitHub Actions" }

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
        Write-Would "download the 8 raw-binary .sha256 and write packaging/npm/checksums.json (launcher integrity pins)"
        Write-Would "commit packaging/{scoop,winget,homebrew,npm/checksums.json} and push origin main"
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

    # Pin the raw-binary hashes into the npm launcher. checksums.json travels inside the
    # immutable npm tarball; the launcher verifies every downloaded binary against it before
    # executing (Socket supply-chain hardening). Keys are the exact raw asset names the
    # launcher fetches: <bin>-<triple>[.exe].
    $binaries = [ordered]@{}
    foreach ($t in $Targets) {
        $suffix = if ($t.Windows) { '.exe' } else { '' }
        foreach ($b in @('ghostlight', 'ghostlight-relay')) {
            $asset = "$b-$($t.Target)$suffix"
            $binaries[$asset] = Get-AssetSha256 $asset $tmp
        }
    }
    $manifest = [ordered]@{ version = $Version; algorithm = 'sha256'; binaries = $binaries }
    $checksumsPath = Join-Path $RepoRoot 'packaging/npm/checksums.json'
    ($manifest | ConvertTo-Json -Depth 5) + "`n" | Set-Content -Path $checksumsPath -Encoding utf8 -NoNewline
    Write-Ok "wrote packaging/npm/checksums.json ($($binaries.Count) launcher integrity pins)"

    Push-Location $RepoRoot
    try {
        $changed = git status --porcelain=v1 -- packaging/scoop packaging/winget packaging/homebrew packaging/npm/checksums.json
        if (-not $changed) {
            Write-Skip 'packaging checksums already up to date; nothing to commit'
            return
        }
        Invoke-Native 'git' @('add', 'packaging/scoop/ghostlight.json', 'packaging/winget/Sylin.Ghostlight.yaml', 'packaging/homebrew/ghostlight.rb', 'packaging/npm/checksums.json') | Out-Null
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

    # Integrity-manifest guard: the launcher verifies every binary against the bundled
    # checksums.json and fails closed without it, so refuse to publish a launcher whose manifest
    # is missing, stale, or short of the 8 pins (that would break every user's first run). The
    # sums step writes it; -From npm without a fresh sums run is the case this catches.
    $checksumsPath = Join-Path $RepoRoot 'packaging/npm/checksums.json'
    if (-not $DryRun) {
        if (-not (Test-Path $checksumsPath)) { throw "packaging/npm/checksums.json is missing; run the 'sums' step first" }
        $cs = Get-Content -Raw $checksumsPath | ConvertFrom-Json
        if ($cs.version -ne $Version) { throw "checksums.json version '$($cs.version)' != $Version; re-run the 'sums' step" }
        $pinCount = @($cs.binaries.PSObject.Properties).Count
        if ($pinCount -ne 8) { throw "checksums.json has $pinCount pins, expected 8; re-run the 'sums' step" }
        Write-Ok "integrity manifest present ($pinCount pins for $Version)"
    }

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

    # Smoke: the launcher fetches the version-matched binary and runs it. First WAIT for the
    # just-published version to propagate through the npm registry -- a smoke fired the instant
    # after `npm publish` reliably hits ETARGET (the publish succeeded; the registry has just not
    # indexed it yet). Poll `npm view` until the version is visible, then run the launcher.
    if ($DryRun) {
        Write-Info "smoke test: npx -y ghostlight@$Version doctor"
        Write-Would "wait for ghostlight@$Version to propagate, then npx -y ghostlight@$Version doctor"
        return
    }

    Write-Info "waiting for ghostlight@$Version to propagate on the npm registry..."
    $propagated = $false
    for ($i = 0; $i -lt 24 -and -not $propagated; $i++) {
        $seen = & npm view "ghostlight@$Version" version 2>$null
        if ($LASTEXITCODE -eq 0 -and $seen -eq $Version) { $propagated = $true; break }
        Start-Sleep -Seconds 5
    }
    if (-not $propagated) {
        Write-Warn2 "ghostlight@$Version not visible on npm after ~2 min; skipping the launcher smoke (the publish still succeeded -- verify with 'npm view ghostlight@$Version version')"
        return
    }
    Write-Ok "ghostlight@$Version visible on npm"

    Write-Info "smoke test: npx -y ghostlight@$Version doctor"
    & npx -y "ghostlight@$Version" doctor
    $code = $LASTEXITCODE
    if ($code -eq 0) { Write-Ok 'doctor healthy (exit 0)' }
    else { Write-Warn2 "doctor exited $code -- the launcher fetched v$Version (see output above); a nonzero here is expected when this machine has no browser/extension configured" }
}

# Derive the DNS domain the registry authenticates against: server.json's name is
# <reverse-dns>/<id> (org.sylin/ghostlight), and the reverse-dns reversed is the domain (sylin.org).
# The apex TXT proof record on that domain must exist for `login dns` to succeed (docs/RELEASE.md).
function Get-RegistryDomain {
    $sj = Get-Content (Join-Path $RepoRoot 'server.json') -Raw | ConvertFrom-Json
    $ns = ($sj.name -split '/')[0]
    $parts = $ns -split '\.'
    [array]::Reverse($parts)
    return ($parts -join '.')
}

# Locate mcp-publisher: prefer one on PATH; else download the pinned release into a temp dir. (The
# tracked release script must not depend on the gitignored local/ copy, so it fetches its own.)
function Resolve-McpPublisher {
    $onPath = Get-Command mcp-publisher -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }
    $arch = if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq [System.Runtime.InteropServices.Architecture]::Arm64) { 'arm64' } else { 'amd64' }
    $os = if ($IsWindows) { 'windows' } elseif ($IsMacOS) { 'darwin' } else { 'linux' }
    $exeName = if ($IsWindows) { 'mcp-publisher.exe' } else { 'mcp-publisher' }
    $dir = Join-Path ([System.IO.Path]::GetTempPath()) "mcp-publisher-$McpPublisherVersion"
    $exe = Join-Path $dir $exeName
    if (-not (Test-Path $exe)) {
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        $asset = "mcp-publisher_${os}_${arch}.tar.gz"
        & gh release download $McpPublisherVersion --repo modelcontextprotocol/registry --pattern $asset --dir $dir --clobber
        if ($LASTEXITCODE -ne 0) { throw "could not download $asset ($McpPublisherVersion) from modelcontextprotocol/registry" }
        & tar -xzf (Join-Path $dir $asset) -C $dir
    }
    if (-not (Test-Path $exe)) { throw "mcp-publisher not found after download at $exe" }
    return $exe
}

function Step-Registry {
    Write-Banner 'Publish to the MCP registry'
    if ($SkipRegistry) { Write-Skip '-SkipRegistry set'; return }

    $domain = Get-RegistryDomain
    $keySet = -not [string]::IsNullOrWhiteSpace($env:MCP_DNS_PRIVATE_KEY)

    if ($DryRun) {
        Write-Would "ensure mcp-publisher $McpPublisherVersion; validate server.json"
        if ($keySet) { Write-Would "login dns --domain $domain, then publish (skips if $Version already on the registry)" }
        else { Write-Warn2 'MCP_DNS_PRIVATE_KEY not set -> would SKIP (set it to auto-publish; see docs/RELEASE.md)' }
        return
    }
    if (-not $keySet) {
        Write-Warn2 'MCP_DNS_PRIVATE_KEY not set -- skipping the registry publish (not fatal).'
        Write-Info 'To automate: set MCP_DNS_PRIVATE_KEY (DNS-auth key; see docs/RELEASE.md) and re-run: -From registry'
        return
    }

    $mcp = Resolve-McpPublisher
    Push-Location $RepoRoot
    try {
        & $mcp validate
        if ($LASTEXITCODE -ne 0) { throw 'server.json failed registry validation (run: mcp-publisher validate)' }
        Write-Ok 'server.json valid'

        & $mcp login dns --domain $domain --private-key $env:MCP_DNS_PRIVATE_KEY | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "mcp-publisher login dns failed (is the apex TXT proof record on $domain live?)" }
        Write-Ok "authenticated via DNS ($domain)"

        $out = (& $mcp publish 2>&1 | Out-String)
        if ($LASTEXITCODE -eq 0) {
            Write-Ok "published to the registry ($RegistryUrl)"
        }
        elseif ($out -match 'duplicate version|already (published|exists)') {
            # The registry is immutable per version; a re-run of the same version is a no-op.
            Write-Skip "version $Version already on the registry; not re-publishing"
        }
        else {
            Write-Host $out
            throw 'mcp-publisher publish failed'
        }
    }
    finally { Pop-Location }
}

# Restamp the "Last reviewed ... against v<x>" version token in every docs/trust/ footer to the
# release version. Idempotent: matches any existing vX.Y.Z (with or without a +dev suffix).
function Set-TrustFooters([string] $Ver) {
    $trustDir = Join-Path $RepoRoot 'docs/trust'
    if (-not (Test-Path $trustDir)) { return @() }
    $touched = [System.Collections.Generic.List[string]]::new()
    foreach ($f in Get-ChildItem $trustDir -Filter '*.md' -File) {
        $text = Get-Content -Raw $f.FullName
        $new = [regex]::Replace($text, '(against )v\d+\.\d+\.\d+(\+dev)?', "`${1}v$Ver")
        if ($new -ne $text) {
            Set-Content -Path $f.FullName -Value $new -NoNewline
            $touched.Add("docs/trust/$($f.Name)")
        }
    }
    return $touched
}

function Step-Trust {
    Write-Banner 'Restamp trust-center footers'
    if ($DryRun) {
        Write-Would "set every docs/trust/*.md 'against v<x>' footer to v$Version, commit + push origin main"
        return
    }
    $touched = @(Set-TrustFooters $Version)
    if ($touched.Count -eq 0) { Write-Skip "trust footers already at v$Version (or none present)"; return }
    Write-Ok "restamped $($touched.Count) trust footer(s) to v$Version"
    Push-Location $RepoRoot
    try {
        $addArgs = @('add') + $touched
        Invoke-Native 'git' $addArgs | Out-Null
        Invoke-Native 'git' @('commit', '-m', "docs(trust): restamp reviewed-against footers to v$Version") | Out-Null
        Invoke-Native 'git' @('push', 'origin', 'main') -AllowFail | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Warn2 'push to origin/main was rejected (protected branch?); the commit is local -- open a PR to land it.'
        }
        else { Write-Ok 'pushed trust-footer restamp to origin/main' }
    }
    finally { Pop-Location }
}

# True if extension/ changed between the previous release tag and this one -- the only case that
# needs a store resubmission (a Rust-only release reuses the store's existing/pending version).
function Test-ExtensionChanged {
    Push-Location $RepoRoot
    try {
        $prev = git tag --list 'v*' --sort=-version:refname | Where-Object { $_ -ne $Tag } | Select-Object -First 1
        if (-not $prev) { return $true } # no prior tag: treat as changed (first submission)
        git diff --quiet "$prev" "$Tag" -- extension/ 2>$null
        return ($LASTEXITCODE -ne 0)
    }
    finally { Pop-Location }
}

function Step-Extension {
    Write-Banner 'Publish the browser extension'
    if ($SkipExtension) { Write-Skip '-SkipExtension set'; return }

    if (-not (Test-ExtensionChanged)) {
        Write-Skip 'extension/ is unchanged since the previous tag; no Chrome Web Store / Edge resubmission needed'
        return
    }
    Write-Info 'extension/ changed this release -> a store resubmission is due'

    $script = Join-Path $PSScriptRoot 'publish-extension.ps1'
    if ($DryRun) {
        Write-Would "pwsh $script -Version $Version -DryRun  (auto-publishes where store creds are set, else prints steps)"
        & pwsh -File $script -Version $Version -DryRun
        return
    }
    # publish-extension.ps1 auto-submits to each store whose credentials are present and prints
    # manual instructions for the rest; it never fails the release for a missing credential.
    & pwsh -File $script -Version $Version
    if ($LASTEXITCODE -ne 0) { Write-Warn2 "publish-extension.ps1 exited $LASTEXITCODE -- review its output above" }
}

function Step-Website {
    Write-Banner 'Refresh the sylin.org website'
    if ($SkipWebsite) { Write-Skip '-SkipWebsite set'; return }

    $script = Join-Path $PSScriptRoot 'publish-website.ps1'
    if ($DryRun) {
        Write-Would "pwsh $script -Version $Version -DryRun  (refresh the install-guide fallback; push only if it changed)"
        & pwsh -File $script -Version $Version -DryRun
        return
    }
    & pwsh -File $script -Version $Version
    if ($LASTEXITCODE -ne 0) { Write-Warn2 "publish-website.ps1 exited $LASTEXITCODE -- review its output above" }
}

function Step-Report {
    Write-Banner 'Done -- summary + remaining manual steps'
    Write-Host @"
  Automated by this script:
    - tag $Tag pushed, Release workflow watched to green
    - assets verified, package-manager checksums filled + committed
    - homebrew tap updated$(if ($SkipTap) { ' (SKIPPED)' })
    - npm publish + smoke$(if ($SkipNpm) { ' (SKIPPED)' })
    - MCP registry publish$(if ($SkipRegistry) { ' (SKIPPED)' }) -- auto when MCP_DNS_PRIVATE_KEY is set, else skipped
    - trust-center footers restamped to v$Version
    - extension published$(if ($SkipExtension) { ' (SKIPPED)' }) -- auto where store creds are set, else steps printed above
    - website install-guide fallback refreshed$(if ($SkipWebsite) { ' (SKIPPED)' })

  Still manual (by nature -- external systems that need a human or a per-version PR):
    - winget: a NEW PR per version to microsoft/winget-pkgs
        (copy the filled packaging/winget/Sylin.Ghostlight.yaml sections; needs the CLA).
    - MCP Registry: mcp-publisher with DNS auth on the sylin.org apex (founder-side).
    - Extension stores: if you have NOT set the store API credentials, follow the steps this
        script printed above (nothing auto-submitted). See docs/RELEASE.md -> "Extension stores".
    - Verify: https://github.com/$RepoSlug/releases/tag/$Tag

  The complete channel-by-channel map is docs/RELEASE.md.
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
    registry  = { Step-Registry }
    trust     = { Step-Trust }
    extension = { Step-Extension }
    website   = { Step-Website }
    report    = { Step-Report }
}

for ($i = $startIndex; $i -lt $StepOrder.Count; $i++) {
    & $dispatch[$StepOrder[$i]]
}

Write-Host ''
Write-Host "Release $Tag orchestration complete." -ForegroundColor Green
