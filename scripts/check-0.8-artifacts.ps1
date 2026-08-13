# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$InventoryPath = "docs/0.8/artifact-inventory.json",
    [string]$RecoveryPath = "docs/0.8/artifact-recovery.json"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$inventoryFile = Join-Path $repo $InventoryPath
$recoveryFile = Join-Path $repo $RecoveryPath
$inventory = Get-Content -LiteralPath $inventoryFile -Raw | ConvertFrom-Json
$recovery = Get-Content -LiteralPath $recoveryFile -Raw | ConvertFrom-Json

if ($inventory.schemaVersion -ne 1 -or $recovery.schemaVersion -ne 1) {
    throw "Unsupported 0.8 artifact schema"
}
if ($inventory.sourceRevision -ne $recovery.sourceRevision) {
    throw "Inventory and recovery revisions differ"
}
if ($inventory.generatedBy -ne "scripts/harvest-0.8-artifacts.ps1" -or
    $recovery.generatedBy -ne "scripts/harvest-0.8-artifacts.ps1") {
    throw "Artifact documents name an unexpected generator"
}

$inventoryEntries = @($inventory.entries | Sort-Object path)
$recoveryEntries = @($recovery.entries | Sort-Object path)
if ($inventoryEntries.Count -ne $recoveryEntries.Count -or
    $inventoryEntries.Count -ne $inventory.counts.total -or
    $recoveryEntries.Count -ne $recovery.counts.total) {
    throw "Artifact entry counts disagree"
}
if ($inventoryEntries.Count -lt 800) {
    throw "Artifact inventory is unexpectedly small: $($inventoryEntries.Count)"
}

$allowedStates = @("absent", "retained-identical", "retained-evolved")
for ($index = 0; $index -lt $inventoryEntries.Count; $index += 1) {
    $observed = $inventoryEntries[$index]
    $planned = $recoveryEntries[$index]
    if ($observed.path -ne $planned.path) {
        throw "Artifact path mismatch at entry $index"
    }
    if ($allowedStates -notcontains $observed.currentState -or
        $observed.currentState -ne $planned.currentState) {
        throw "Artifact state mismatch for $($observed.path)"
    }
    if ([string]::IsNullOrWhiteSpace($observed.historicalBlob) -or
        [string]::IsNullOrWhiteSpace($planned.treatment) -or
        [string]::IsNullOrWhiteSpace($planned.reason)) {
        throw "Artifact has an incomplete disposition: $($observed.path)"
    }
}

$tempBase = [System.IO.Path]::GetTempPath()
$tempRoot = Join-Path $tempBase ("ghostlight-artifact-harvest-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tempRoot | Out-Null
try {
    $freshInventory = Join-Path $tempRoot "artifact-inventory.json"
    $freshRecovery = Join-Path $tempRoot "artifact-recovery.json"
    & (Join-Path $PSScriptRoot "harvest-0.8-artifacts.ps1") `
        -Revision $inventory.sourceRevision `
        -InventoryPath $freshInventory `
        -RecoveryPath $freshRecovery | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Artifact harvest failed"
    }

    foreach ($pair in @(
        @{ Expected = $inventoryFile; Actual = $freshInventory },
        @{ Expected = $recoveryFile; Actual = $freshRecovery }
    )) {
        $expectedHash = (Get-FileHash -LiteralPath $pair.Expected -Algorithm SHA256).Hash
        $actualHash = (Get-FileHash -LiteralPath $pair.Actual -Algorithm SHA256).Hash
        if ($expectedHash -ne $actualHash) {
            throw "Generated artifact ledger has drifted: $($pair.Expected)"
        }
    }
}
finally {
    $resolved = [System.IO.Path]::GetFullPath($tempRoot)
    if (-not $resolved.StartsWith(
            [System.IO.Path]::GetFullPath($tempBase),
            [System.StringComparison]::OrdinalIgnoreCase
        ) -or
        -not ([System.IO.Path]::GetFileName($resolved)).StartsWith("ghostlight-artifact-harvest-")) {
        throw "Refusing to clean unexpected artifact-harvest path: $resolved"
    }
    if (Test-Path -LiteralPath $resolved) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

Write-Output "All $($inventoryEntries.Count) historical artifacts have an explicit checked disposition."

