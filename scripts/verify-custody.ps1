# SPDX-License-Identifier: Apache-2.0 OR MIT

<#
.SYNOPSIS
    Verify one assembled candidate directory against an independently supplied source
    revision and its own checksums, then delegate to the deep candidate checks.

.DESCRIPTION
    The custody half of G2, in one command. Runs three steps:

      1. Source revision: the candidate manifest's sourceRevision must equal the commit
         that -Revision resolves to.
      2. Deep candidate checks: scripts/check-release-candidate.ps1 (manifest shape, exact
         artifact roster and coordinates). It rehashes every asset from bytes on disk and
         requires SHA256SUMS to equal the sorted manifest exactly.
      3. Custody instruction: the verified directory must be copied somewhere local and this
         command re-run against the copy before anything publishes or submits.

    Provenance attestation verification against GitHub requires network access and gh
    authentication; run it with -IncludeProvenance when that is appropriate.

.PARAMETER CandidateDirectory
    Directory containing release-candidate.json, SHA256SUMS, and assets/.

.PARAMETER Revision
    A commit or a tag naming the revision the candidate must have been built from. The
    value must come from the dispatched workflow run's head SHA (for example
    gh run view <run-id> --json headSha) or from the release tag, never copied from the
    candidate's own release-candidate.json.

.PARAMETER IncludeProvenance
    Additionally verify GitHub artifact attestations for every raw asset via gh CLI,
    bound to the resolved source revision.

.EXAMPLE
    pwsh scripts/verify-custody.ps1 build/release-candidate -Revision <sha or tag> -IncludeProvenance
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $CandidateDirectory,
    [Parameter(Mandatory)] [string] $Revision,
    [switch] $IncludeProvenance
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repository = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$candidateDirectory = [System.IO.Path]::GetFullPath($CandidateDirectory)

function Assert-Step {
    param([Parameter(Mandatory)] [string] $Name, [Parameter(Mandatory)] [scriptblock] $Action)
    try {
        & $Action
        Write-Host ("{0,-58} PASS" -f $Name)
    } catch {
        Write-Host ("{0,-58} FAIL   {1}" -f $Name, $_.Exception.Message)
        exit 1
    }
}

Write-Host "Custody verification for $candidateDirectory"

$expectedRevision = ""
Assert-Step "source revision" {
    $resolved = (git -C $repository rev-parse --verify "$Revision^{commit}" 2>$null)
    if ($LASTEXITCODE -ne 0 -or -not $resolved) {
        throw "-Revision '$Revision' does not resolve to a commit in this repository; fetch it or pass a full SHA or tag."
    }
    $script:expectedRevision = "$resolved".Trim()
    $manifestPath = Join-Path $candidateDirectory "release-candidate.json"
    if (-not (Test-Path -LiteralPath $manifestPath)) { throw "candidate manifest missing" }
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.sourceRevision -ne $script:expectedRevision) {
        throw ("sourceRevision mismatch: expected={0} candidate={1}" -f $script:expectedRevision, $manifest.sourceRevision)
    }
}

Assert-Step "deep candidate checks (check-release-candidate)" {
    & (Join-Path $repository "scripts/check-release-candidate.ps1") -CandidateDirectory $candidateDirectory
    if ($LASTEXITCODE -ne 0) { throw "deep checks reported failures" }
}

if ($IncludeProvenance) {
    Assert-Step "GitHub provenance attestation (raw binaries)" {
        $assets = Join-Path $candidateDirectory "assets"
        foreach ($name in @(
            "ghostlight-x86_64-pc-windows-msvc.exe",
            "ghostlight-mcp-connector-x86_64-pc-windows-msvc.exe",
            "ghostlight-browser-connector-x86_64-pc-windows-msvc.exe",
            "ghostlight-x86_64-unknown-linux-gnu",
            "ghostlight-mcp-connector-x86_64-unknown-linux-gnu",
            "ghostlight-browser-connector-x86_64-unknown-linux-gnu"
        )) {
            gh attestation verify (Join-Path $assets $name) -R sylin-org/ghostlight --source-digest $script:expectedRevision
            if ($LASTEXITCODE -ne 0) { throw "provenance failed for $name" }
        }
    }
} else {
    Write-Host ("{0,-58} SKIP   re-run with -IncludeProvenance once artifacts are on GitHub" -f "GitHub provenance attestation")
}

Write-Host ""
Write-Host "Custody instruction: copy this directory somewhere local, re-run this command"
Write-Host "against the copy with the same -Revision, and only then treat the candidate as held."
Write-Host "Nothing publishes or submits until G3 rows are checked by the owner."
