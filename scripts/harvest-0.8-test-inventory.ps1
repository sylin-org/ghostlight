# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$Revision = "c01cc3276102471f3e18de2ae90cb90abf98ed88",
    [string]$OutputPath = "docs/0.8/test-inventory.json"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Read-HistoricalFile {
    param([string]$Path)

    $text = (& git show "${Revision}:$Path") -join "`n"
    if ($LASTEXITCODE -ne 0) {
        throw "Could not read $Path at $Revision"
    }
    return $text
}

function Get-Category {
    param([string]$Path)

    switch -Regex ($Path) {
        '^tests/extension/|^extension/tests/' { return "extension" }
        '^tests/e2e/' { return "browser-e2e" }
        '^crates/lightbox/' { return "lightbox" }
        '^crates/core/src/governance/' { return "governance" }
        '^crates/core/src/hub/' { return "browser-hub" }
        '^crates/core/src/install/' { return "installation" }
        '^crates/core/src/tool/|^crates/core/src/operation/' { return "tool-execution" }
        '^crates/transport/' { return "transport" }
        '^crates/mcp-connector/' { return "mcp-edge" }
        '^tests/' { return "integration" }
        default { return "supporting-unit" }
    }
}

$inventory = [System.Collections.Generic.List[object]]::new()
$paths = & git ls-tree -r --name-only $Revision -- crates tests extension
if ($LASTEXITCODE -ne 0) {
    throw "Could not list test-bearing files at $Revision"
}

foreach ($path in $paths) {
    if ($path -notmatch '\.(rs|js|mjs)$') {
        continue
    }

    $text = Read-HistoricalFile -Path $path
    $category = Get-Category -Path $path

    if ($path.EndsWith(".rs")) {
        $matches = [regex]::Matches(
            $text,
            '(?ms)^\s*#\[(?:[A-Za-z0-9_]+::)?test(?:\([^]]*\))?\]\s*(?:(?:#\[[^]]+\]|///[^\r\n]*)\s*)*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(?<name>[A-Za-z0-9_]+)'
        )
        $ordinal = 0
        foreach ($match in $matches) {
            $ordinal += 1
            $name = $match.Groups["name"].Value
            if ($name -match '_plist_') {
                continue
            }
            [void]$inventory.Add([pscustomobject][ordered]@{
                kind = "rust-test"
                category = $category
                file = $path
                name = $name
                ordinalInFile = $ordinal
            })
        }
    }
    else {
        $matches = [regex]::Matches(
            $text,
            '(?ms)^\s*(?:test|it)\s*\(\s*["''](?<name>[^"'']+)["'']'
        )
        $ordinal = 0
        foreach ($match in $matches) {
            $ordinal += 1
            [void]$inventory.Add([pscustomobject][ordered]@{
                kind = "javascript-test"
                category = $category
                file = $path
                name = $match.Groups["name"].Value
                ordinalInFile = $ordinal
            })
        }
    }
}

$scenarioFiles = @(
    "crates/lightbox/src/scenarios.rs",
    "crates/lightbox/src/mechanism_wire.rs",
    "crates/lightbox/src/legacy/mod.rs",
    "crates/lightbox/src/legacy/browser.rs",
    "crates/lightbox/src/legacy/console.rs",
    "crates/lightbox/src/legacy/hub.rs",
    "crates/lightbox/src/legacy/lifecycle.rs",
    "crates/lightbox/src/legacy/policy.rs"
)

foreach ($path in $scenarioFiles) {
    $text = Read-HistoricalFile -Path $path
    $matches = [regex]::Matches(
        $text,
        '(?ms)\(\s*"(?<name>[a-z0-9_-]+)"\s*,\s*(?<function>[a-zA-Z_][a-zA-Z0-9_]*)'
    )
    $ordinal = 0
    foreach ($match in $matches) {
        $name = $match.Groups["name"].Value
        if ($name -eq "read_page") {
            continue
        }
        $ordinal += 1
        [void]$inventory.Add([pscustomobject][ordered]@{
            kind = "lightbox-scenario"
            category = "process-contract"
            file = $path
            name = $name
            ordinalInFile = $ordinal
        })
    }
}

$orderedInventory = @($inventory | Sort-Object kind, category, file, ordinalInFile, name)
$testCount = @($orderedInventory | Where-Object { $_.kind -ne "lightbox-scenario" }).Count
$scenarioCount = @($orderedInventory | Where-Object { $_.kind -eq "lightbox-scenario" }).Count
$categoryCounts = [ordered]@{}
foreach ($group in ($orderedInventory | Group-Object category | Sort-Object Name)) {
    $categoryCounts[$group.Name] = $group.Count
}

$document = [ordered]@{
    schemaVersion = 1
    sourceRevision = $Revision
    generatedBy = "scripts/harvest-0.8-test-inventory.ps1"
    counts = [ordered]@{
        decoratedOrJavascriptTests = $testCount
        lightboxScenarios = $scenarioCount
        totalEntries = $orderedInventory.Count
        byCategory = $categoryCounts
    }
    notes = @(
        "This is an evidence inventory, not a requirement that every old mechanism return.",
        "Each applicable behavior must be re-expressed through the current 1.0 architecture.",
        "The historical Lightbox ledger says 37 scenarios, while the source registries enumerate 34; both facts are preserved in docs/0.8/HARVEST.md."
    )
    entries = $orderedInventory
}

$parent = Split-Path -Parent $OutputPath
if (-not (Test-Path -LiteralPath $parent)) {
    New-Item -ItemType Directory -Path $parent | Out-Null
}
$json = $document | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText(
    (Join-Path (Get-Location) $OutputPath),
    $json + "`n",
    [System.Text.UTF8Encoding]::new($false)
)

Write-Output "Wrote $OutputPath with $testCount tests and $scenarioCount Lightbox scenarios."
