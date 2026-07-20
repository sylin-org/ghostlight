#Requires -Version 7
<#
.SYNOPSIS
    Refresh the sylin.org website's Ghostlight fallbacks and trigger a rebuild, so the published
    site tracks a new release and its verified public status.

.DESCRIPTION
    The website (sylin-org/website, an Eleventy site) is DESIGNED to auto-track this repo: at build
    time src/_data/ghostlightInstall.js fetches llms-install.md live from ghostlight's main branch
    and republishes it at https://sylin.org/ghostlight/install.md. It keeps a committed fallback
    snapshot beside that loader as a safety net for when the live fetch fails. The demo pages are
    static, and the install guide is version-agnostic (no version/download strings to bump), so a
    release needs no per-version content edits.

    The project page also consumes a committed public-status fallback owned by this repository.
    It carries the current release, live-platform evidence, and extension-store state. A release
    needs both fallbacks kept fresh and a website rebuild so the live install fetch re-runs. The
    site deploys via an external host that builds on push to the website repo's main, so this script:
      - clones the website repo,
      - copies this repo's install guide and public status over the committed fallbacks,
      - commits + pushes IF either fallback changed (which triggers the rebuild).
    If both fallbacks are unchanged, the live site already serves the current guide and status; no rebuild is
    needed -- pass -ForceRebuild to push an empty commit and rebuild anyway.

    This never edits the website's demo pages or layout; it touches ONLY the fallback snapshot.

.PARAMETER Version
    The release version, used only in the commit message. Defaults to extension/manifest.json.

.PARAMETER WebsiteSlug
    The website repo (owner/name). Default: sylin-org/website.

.PARAMETER ForceRebuild
    Push an empty commit to trigger a rebuild even when the fallback snapshot did not change.

.PARAMETER DryRun
    Report what would happen (including whether the fallback would change) without pushing.

.EXAMPLE
    pwsh -File scripts/publish-website.ps1 -Version 0.5.6
#>
[CmdletBinding()]
param(
    [string] $Version,
    [string] $WebsiteSlug = 'sylin-org/website',
    [switch] $ForceRebuild,
    [switch] $DryRun
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

function Write-Banner([string] $Text) { Write-Host ''; Write-Host "=== $Text ===" -ForegroundColor Cyan }
function Write-Ok([string] $Text) { Write-Host "  [ok]   $Text" -ForegroundColor Green }
function Write-Skip([string] $Text) { Write-Host "  [skip] $Text" -ForegroundColor DarkGray }
function Write-Info([string] $Text) { Write-Host "  [info] $Text" }
function Write-Would([string] $Text) { Write-Host "  [dry]  would: $Text" -ForegroundColor Magenta }

function Resolve-Version {
    if ($Version) { return $Version }
    $manifest = Get-Content (Join-Path $RepoRoot 'extension/manifest.json') -Raw | ConvertFrom-Json
    return $manifest.version
}

# One trailing newline, matching the site loader's own normalize().
function Get-Normalized([string] $Path) {
    $content = (Get-Content -Raw $Path) -replace '\r\n?', "`n"
    return ($content -replace '\s*$', '') + "`n"
}

Write-Banner 'Website (sylin.org) refresh'

$installSource = Join-Path $RepoRoot 'llms-install.md'
$statusSource = Join-Path $RepoRoot 'docs/public-status.json'
if (-not (Test-Path $installSource)) { throw "llms-install.md not found at $installSource (the site's install guide source)" }
if (-not (Test-Path $statusSource)) { throw "docs/public-status.json not found at $statusSource (the site's public-status source)" }

foreach ($tool in @('git', 'gh')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "required tool not found: $tool" }
}

$ver = Resolve-Version
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) "ghostlight-website-$ver"
if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }

Write-Info "cloning $WebsiteSlug"
& gh repo clone $WebsiteSlug $tmp -- --depth 1
if ($LASTEXITCODE -ne 0) { throw "failed to clone $WebsiteSlug" }

$installFallback = Join-Path $tmp 'src/_data/ghostlight-install.fallback.md'
$statusFallback = Join-Path $tmp 'src/_data/ghostlight-public-status.fallback.json'
if (-not (Test-Path $installFallback)) {
    throw "install fallback not found at src/_data/ghostlight-install.fallback.md in $WebsiteSlug (site layout changed?)"
}
if (-not (Test-Path $statusFallback)) {
    throw "public-status fallback not found at src/_data/ghostlight-public-status.fallback.json in $WebsiteSlug (site layout changed?)"
}

$installNew = Get-Normalized $installSource
$installOld = Get-Normalized $installFallback
$statusNew = Get-Normalized $statusSource
$statusOld = Get-Normalized $statusFallback
$installChanged = ($installNew -ne $installOld)
$statusChanged = ($statusNew -ne $statusOld)
$changed = $installChanged -or $statusChanged

if ($changed) {
    if ($installChanged) { Write-Info 'the install guide changed since the committed fallback snapshot' }
    if ($statusChanged) { Write-Info 'the public status changed since the committed fallback snapshot' }
    if ($DryRun) { Write-Would "write the refreshed fallback(s), commit, and push (triggers a site rebuild)"; return }
    if ($installChanged) { [System.IO.File]::WriteAllText($installFallback, $installNew) }
    if ($statusChanged) { [System.IO.File]::WriteAllText($statusFallback, $statusNew) }
}
else {
    Write-Info 'the fallback snapshots already match this repo''s install guide and public status'
    if (-not $ForceRebuild) {
        Write-Skip 'nothing to refresh; the live site already serves the current guide (use -ForceRebuild to rebuild anyway)'
        return
    }
    if ($DryRun) { Write-Would "push an empty commit to trigger a rebuild (-ForceRebuild)"; return }
}

Push-Location $tmp
try {
    if ($changed) {
        & git add 'src/_data/ghostlight-install.fallback.md' 'src/_data/ghostlight-public-status.fallback.json'
        & git commit -m "chore(ghostlight): refresh public fallbacks for v$ver"
    }
    else {
        # -ForceRebuild with no content change: an empty commit is the host-agnostic rebuild trigger.
        & git commit --allow-empty -m "chore(ghostlight): trigger rebuild for v$ver"
    }
    if ($LASTEXITCODE -ne 0) { throw 'commit failed' }
    & git push
    if ($LASTEXITCODE -ne 0) { throw 'push to the website repo failed' }
    Write-Ok "pushed to $WebsiteSlug -- the external host rebuilds on push and serves the current Ghostlight truth"
}
finally { Pop-Location }

Write-Host ''
Write-Host 'Website refresh complete.' -ForegroundColor Green
