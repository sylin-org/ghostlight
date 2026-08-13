# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$InventoryPath = "docs/0.8/test-inventory.json",
    [string]$RecoveryPath = "docs/0.8/test-recovery.json"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$inventoryFile = Join-Path $repo $InventoryPath
$recoveryFile = Join-Path $repo $RecoveryPath
$inventory = Get-Content -LiteralPath $inventoryFile -Raw | ConvertFrom-Json
$recovery = Get-Content -LiteralPath $recoveryFile -Raw | ConvertFrom-Json

if ($inventory.schemaVersion -ne 1 -or $recovery.schemaVersion -ne 1) {
    throw "Unsupported 0.8 recovery schema"
}
if ($inventory.sourceRevision -ne $recovery.sourceRevision) {
    throw "Recovery source revision does not match the harvested inventory"
}

$inventoryCategories = @($inventory.entries | Group-Object category | Sort-Object Name)
$recoveryCategories = @($recovery.categoryCoverage | Sort-Object category)
if ($inventoryCategories.Count -ne $recoveryCategories.Count) {
    throw "Recovery matrix does not cover every inventory category"
}

$covered = 0
for ($index = 0; $index -lt $inventoryCategories.Count; $index += 1) {
    $observed = $inventoryCategories[$index]
    $planned = $recoveryCategories[$index]
    if ($observed.Name -ne $planned.category) {
        throw "Recovery category mismatch: $($observed.Name) vs $($planned.category)"
    }
    if ($observed.Count -ne $planned.historicalEntries) {
        throw "Recovery count for $($planned.category) is stale"
    }
    if ([string]::IsNullOrWhiteSpace($planned.treatment) -or
        [string]::IsNullOrWhiteSpace($planned.remainingProof)) {
        throw "Recovery category $($planned.category) has no treatment or remaining proof"
    }
    foreach ($evidence in $planned.currentEvidence) {
        if (-not (Test-Path -LiteralPath (Join-Path $repo $evidence))) {
            throw "Recovery evidence does not exist: $evidence"
        }
    }
    $covered += $observed.Count
}
if ($covered -ne $inventory.counts.totalEntries) {
    throw "Recovery matrix covers $covered entries, expected $($inventory.counts.totalEntries)"
}

$allowedStatuses = @(
    "reexpressed",
    "superseded",
    "superseded-invariant-retained",
    "deferred",
    "live-gate"
)
$inventoryScenarios = @(
    $inventory.entries |
        Where-Object kind -eq "lightbox-scenario" |
        Select-Object -ExpandProperty name |
        Sort-Object
)
$recoveryScenarios = @($recovery.processScenarios | Sort-Object name)
if ($inventoryScenarios.Count -ne $recoveryScenarios.Count) {
    throw "Recovery matrix does not cover every Lightbox scenario"
}
for ($index = 0; $index -lt $inventoryScenarios.Count; $index += 1) {
    $scenario = $recoveryScenarios[$index]
    if ($inventoryScenarios[$index] -ne $scenario.name) {
        throw "Lightbox scenario mismatch: $($inventoryScenarios[$index]) vs $($scenario.name)"
    }
    if ($allowedStatuses -notcontains $scenario.status) {
        throw "Unknown recovery status for $($scenario.name): $($scenario.status)"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $repo $scenario.evidence))) {
        throw "Scenario evidence does not exist for $($scenario.name): $($scenario.evidence)"
    }
}

$statusCounts = $recovery.processScenarios | Group-Object status | Sort-Object Name
Write-Output "0.8 recovery covers all $covered inventory entries in $($recoveryCategories.Count) reviewed groups."
Write-Output "All $($inventoryScenarios.Count) Lightbox scenarios have an explicit disposition:"
foreach ($status in $statusCounts) {
    Write-Output "  $($status.Name): $($status.Count)"
}
