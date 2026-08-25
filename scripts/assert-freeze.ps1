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
        # The freeze commit that carries docs/release/freeze.json necessarily lands after the
        # frozen source revision, so HEAD may sit ahead of it -- as long as it is a descendant
        # and no product or packaging path has moved. Docs-only descendants pass; any product
        # diff reopens the freeze.
        $ancestor = git merge-base --is-ancestor $freeze.revision $resolved 2>$null
        if ($LASTEXITCODE -ne 0) {
            Write-Host ("FAIL: revision mismatch and not a descendant. freeze={0} actual={1}" -f $freeze.revision, $resolved)
            exit 1
        }
        $productPaths = @("crates", "extension", "packaging", "examples", "site", "Cargo.toml", "Cargo.lock")
        $changed = (git diff --name-only "$($freeze.revision)..$resolved" -- @productPaths) | Where-Object { $_ }
        if ($changed) {
            Write-Host ("FAIL: product or packaging paths changed since the frozen revision:")
            $changed | ForEach-Object { Write-Host "  $_" }
            exit 1
        }
        Write-Host ("Freeze binding verified: {0} (HEAD {1} is a docs-only descendant)" -f $freeze.revision, $resolved)
        exit 0
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
