# SPDX-License-Identifier: Apache-2.0 OR MIT

<#
.SYNOPSIS
    Assert that a revision matches the declared G0 freeze.

.DESCRIPTION
    Reads docs/release/freeze.json and compares its revision against the current HEAD (or the
    explicitly supplied revision). Exits non-zero on any mismatch, on a dirty tree, or when no
    freeze has been declared. Intended as the first step of every downstream release
    verification: candidate assembly, custody verification, store byte-matching.

.PARAMETER Revision
    Compare against this revision instead of the current HEAD.

.EXAMPLE
    pwsh scripts/assert-freeze.ps1
#>

[CmdletBinding()]
param(
    [string] $Revision = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repository = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Push-Location $repository
try {
    $freezePath = Join-Path $repository "docs/release/freeze.json"
    if (-not (Test-Path -LiteralPath $freezePath)) {
        Write-Host "FAIL: no freeze declared. At G0 run scripts/declare-freeze.ps1 and commit docs/release/freeze.json."
        exit 1
    }
    $freeze = Get-Content -LiteralPath $freezePath -Raw | ConvertFrom-Json
    if (-not $freeze.revision) {
        Write-Host "FAIL: freeze declaration carries no revision."
        exit 1
    }

    if (-not $Revision) {
        $Revision = (git rev-parse HEAD).Trim()
    }
    $resolved = (git rev-parse --verify "$Revision^{commit}").Trim()

    if ($resolved -ne $freeze.revision) {
        Write-Host ("FAIL: revision mismatch. freeze={0} actual={1}" -f $freeze.revision, $resolved)
        exit 1
    }

    $dirtyLines = (git status --porcelain | Measure-Object -Line).Lines
    if ($dirtyLines -gt 0) {
        Write-Host "FAIL: the tree is dirty ($dirtyLines entries); a frozen revision must be verified from a clean checkout."
        exit 1
    }

    Write-Host "Freeze binding verified: $resolved"
    exit 0
}
finally {
    Pop-Location
}
