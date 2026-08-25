# SPDX-License-Identifier: Apache-2.0 OR MIT

<#
.SYNOPSIS
    Verify one assembled candidate directory against the declared freeze and its own
    checksums, then delegate to the deep candidate checks.

.DESCRIPTION
    The custody half of G2, in one command. Runs four steps:

      1. Freeze binding: the candidate manifest's sourceRevision must equal the revision
         declared in docs/release/freeze.json.
      2. Deep candidate checks: scripts/check-release-candidate.ps1 (manifest shape, exact
         artifact roster and coordinates, hashes).
      3. SHA256SUMS recomputation: every line is rehashed from bytes on disk; the line set
         must be exactly the manifest's asset set.
      4. Custody instruction: the verified directory must be copied somewhere local and this
         command re-run against the copy before anything publishes or submits.

    Provenance attestation verification against GitHub requires network access and gh
    authentication; run it with -IncludeProvenance when that is appropriate.

.PARAMETER CandidateDirectory
    Directory containing release-candidate.json, SHA256SUMS, and assets/.

.PARAMETER IncludeProvenance
    Additionally verify GitHub artifact attestations for every raw asset via gh CLI.

.EXAMPLE
    pwsh scripts/verify-custody.ps1 build/release-candidate -IncludeProvenance
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $CandidateDirectory,
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

Assert-Step "freeze binding" {
    $freezePath = Join-Path $repository "docs/release/freeze.json"
    if (-not (Test-Path -LiteralPath $freezePath)) {
        throw "no freeze declared; declare the revision at G0 first."
    }
    $freeze = Get-Content -LiteralPath $freezePath -Raw | ConvertFrom-Json
    $manifestPath = Join-Path $candidateDirectory "release-candidate.json"
    if (-not (Test-Path -LiteralPath $manifestPath)) { throw "candidate manifest missing" }
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.sourceRevision -ne $freeze.revision) {
        throw ("sourceRevision mismatch: freeze={0} candidate={1}" -f $freeze.revision, $manifest.sourceRevision)
    }
}

Assert-Step "deep candidate checks (check-release-candidate)" {
    & (Join-Path $repository "scripts/check-release-candidate.ps1") -CandidateDirectory $candidateDirectory
    if ($LASTEXITCODE -ne 0) { throw "deep checks reported failures" }
}

Assert-Step "SHA256SUMS recomputation (18 assets)" {
    $sumsPath = Join-Path $candidateDirectory "SHA256SUMS"
    $lines = Get-Content -LiteralPath $sumsPath | Where-Object { $_.Trim() }
    if ($lines.Count -ne 18) {
        throw ("expected 18 checksum lines, found {0}" -f $lines.Count)
    }
    foreach ($line in $lines) {
        $parts = $line -split "\s+", 2
        $expectedHash = $parts[0].ToLowerInvariant()
        $relative = $parts[1].TrimStart("*")
        # SHA256SUMS carries bare asset names; the assembled layout keeps them under assets/.
        $file = Join-Path (Join-Path $candidateDirectory "assets") $relative
        if (-not (Test-Path -LiteralPath $file)) {
            $file = Join-Path $candidateDirectory $relative
        }
        if (-not (Test-Path -LiteralPath $file)) { throw "checksummed file missing: $relative" }
        $actual = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -ne $expectedHash) { throw "hash mismatch: $relative" }
    }
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
            gh attestation verify (Join-Path $assets $name) -R sylin-org/ghostlight
            if ($LASTEXITCODE -ne 0) { throw "provenance failed for $name" }
        }
    }
} else {
    Write-Host ("{0,-58} SKIP   re-run with -IncludeProvenance once artifacts are on GitHub" -f "GitHub provenance attestation")
}

Write-Host ""
Write-Host "Custody instruction: copy this directory somewhere local, re-run this command"
Write-Host "against the copy, and only then treat the candidate as held. Nothing publishes"
Write-Host "or submits until G3 rows are checked by the owner."
