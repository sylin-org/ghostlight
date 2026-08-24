# SPDX-License-Identifier: Apache-2.0 OR MIT

<#
.SYNOPSIS
    Declare the G0 frozen revision: write docs/release/freeze.json bound to one commit.

.DESCRIPTION
    G0 names exactly one source revision. This script records it in a machine-readable file
    that every downstream verifier (preflight stage, candidate assembly, custody check) can
    assert against, replacing hand-copied SHAs. The file must be committed; until it is, the
    declaration exists only on this machine.

.PARAMETER Revision
    The revision to freeze. Defaults to the current HEAD. Accepts anything `git rev-parse`
    resolves to a commit.

.PARAMETER Note
    Optional one-line note recorded alongside the revision.

.PARAMETER Force
    Allow replacing an existing freeze declaration.

.EXAMPLE
    pwsh scripts/declare-freeze.ps1 -Revision 93bdcaef -Note "1.0 release candidate"
#>

[CmdletBinding()]
param(
    [string] $Revision = "HEAD",
    [string] $Note = "",
    [switch] $Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repository = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Push-Location $repository
try {
    $full = (git rev-parse --verify "$Revision^{commit}").Trim()
    if ($LASTEXITCODE -ne 0) { throw "revision does not resolve to a commit: $Revision" }

    $freezePath = Join-Path $repository "docs/release/freeze.json"
    if ((Test-Path -LiteralPath $freezePath) -and -not $Force) {
        throw "freeze already declared at $freezePath; pass -Force to replace it deliberately."
    }

    $frozenAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $frozenBy = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name.Split("\")[-1]
    $document = [ordered]@{
        schemaVersion = 1
        revision      = $full
        frozen_at_utc = $frozenAt
        frozen_by     = $frozenBy
    }
    if ($Note) { $document["note"] = $Note }

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $freezePath) | Out-Null
    $json = $document | ConvertTo-Json
    [System.IO.File]::WriteAllText($freezePath, ($json -replace "\r?\n", "`n"))

    Write-Host "Freeze declared:"
    Write-Host "  revision: $full"
    Write-Host "  recorded: $freezePath"
    Write-Host "Next: commit this file. Downstream verifiers (release-preflight, verify-custody)"
    Write-Host "will refuse to proceed from any other revision."
}
finally {
    Pop-Location
}
