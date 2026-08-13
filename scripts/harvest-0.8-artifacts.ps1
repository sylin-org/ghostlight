# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$Revision = "c01cc3276102471f3e18de2ae90cb90abf98ed88",
    [string]$InventoryPath = "docs/0.8/artifact-inventory.json",
    [string]$RecoveryPath = "docs/0.8/artifact-recovery.json"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Get-HistoricalEntries {
    $lines = @(& git ls-tree -r $Revision)
    if ($LASTEXITCODE -ne 0) {
        throw "Could not list the historical tree at $Revision"
    }

    foreach ($line in $lines) {
        $match = [regex]::Match(
            $line,
            '^(?<mode>[0-9]+)\s+(?<type>\S+)\s+(?<blob>[0-9a-f]+)\t(?<path>.+)$'
        )
        if (-not $match.Success) {
            throw "Could not parse git tree entry: $line"
        }
        [pscustomobject][ordered]@{
            path = $match.Groups["path"].Value
            mode = $match.Groups["mode"].Value
            type = $match.Groups["type"].Value
            blob = $match.Groups["blob"].Value
        }
    }
}

function Get-CurrentBlob {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $null
    }
    $blob = (& git hash-object -- $Path).Trim()
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($blob)) {
        throw "Could not hash current file: $Path"
    }
    return $blob
}

function Get-Area {
    param([string]$Path)

    switch -Regex ($Path) {
        '^\.github/' { return "automation" }
        '^\.cargo/|^Cargo\.|^deny\.toml$|^rust-toolchain' { return "build-policy" }
        '^crates/' { return "implementation" }
        '^extension/' { return "browser-adapter" }
        '^tests/' { return "verification" }
        '^scripts/|^build\.' { return "developer-and-release-tooling" }
        '^packaging/' { return "distribution" }
        '^docs/|^README|^CHANGELOG|^CONTRIBUTING|^SECURITY|^LICENSE|^CODE_OF_CONDUCT' {
            return "documentation-and-identity"
        }
        '^site/|^open-spec/|^examples/' { return "public-surface" }
        default { return "repository-support" }
    }
}

function Get-MissingDisposition {
    param([string]$Path)

    $exact = @{
        ".cargo/config.toml" = @(
            "restore-lean",
            "Restore the clean-Windows static CRT lesson and prove it through package installation."
        )
        "scripts/dev-loop.ps1" = @(
            "restore-lean",
            "Restore one checked live-stack replacement command around the current demand-start topology."
        )
        "scripts/get-cws-refresh-token.ps1" = @(
            "restore-lean",
            "Restore the one-time Chrome OAuth recovery helper independently of publication."
        )
        "scripts/publish-extension.ps1" = @(
            "restore-lean",
            "Restore Chrome publication as an independent explicit command with a non-mutating default."
        )
        "scripts/release.ps1" = @(
            "retired-low-value-conductor",
            "Do not restore the 901-line cross-channel conductor; retain its safeguards in small commands."
        )
        "scripts/publish-website.ps1" = @(
            "superseded-by-current-workflow",
            "The tracked Pages workflow owns website deployment; release publication must not rewrite another tree."
        )
    }
    if ($exact.ContainsKey($Path)) {
        return $exact[$Path]
    }

    switch -Regex ($Path) {
        '^tests/|/tests?/|^crates/lightbox/' {
            return @(
                "behavior-dispositioned",
                "The assertion belongs to the checked 0.8 behavior inventory and current recovery matrix."
            )
        }
        '^crates/|^src/|^extension/' {
            return @(
                "historical-implementation-only",
                "Keep the old implementation in Git history; translate observable behavior onto current 1.0 seams."
            )
        }
        '^packaging/|^scripts/(get\.(ps1|sh)|package-mcpb\.mjs|prepare-winget\.ps1)$' {
            return @(
                "redesign-after-signed-candidate",
                "The raw-binary 0.8 distribution shape cannot be relabeled as the 1.0 native desktop package."
            )
        }
        '^\.github/' {
            return @(
                "reexpressed-in-current-automation",
                "Retain the safeguard in the current CI, candidate, dependency, or publication workflow where applicable."
            )
        }
        '^scripts/' {
            return @(
                "reviewed-tooling-history",
                "The tool remains recoverable from Git and is restored only where it serves the current release unit."
            )
        }
        '^docs/|^README|^CHANGELOG|^CONTRIBUTING|^SECURITY|^LICENSE' {
            return @(
                "historical-document-in-git",
                "The historical document remains recoverable at the named source revision and is not active 1.0 authority."
            )
        }
        default {
            return @(
                "reviewed-repository-history",
                "The artifact remains named and content-addressed here even though it has no current-tree path."
            )
        }
    }
}

$historical = @(Get-HistoricalEntries | Sort-Object path)
$inventoryEntries = [System.Collections.Generic.List[object]]::new()
$recoveryEntries = [System.Collections.Generic.List[object]]::new()

foreach ($entry in $historical) {
    $currentBlob = Get-CurrentBlob -Path $entry.path
    $state = if ($null -eq $currentBlob) {
        "absent"
    }
    elseif ($currentBlob -eq $entry.blob) {
        "retained-identical"
    }
    else {
        "retained-evolved"
    }

    [void]$inventoryEntries.Add([pscustomobject][ordered]@{
        path = $entry.path
        area = Get-Area -Path $entry.path
        historicalMode = $entry.mode
        historicalType = $entry.type
        historicalBlob = $entry.blob
        currentState = $state
        currentBlob = $currentBlob
    })

    if ($state -eq "retained-identical") {
        $treatment = "retained-identical"
        $reason = "The current tree retains the exact historical blob at the same path."
    }
    elseif ($state -eq "retained-evolved") {
        $treatment = "retained-evolved"
        $reason = "The path remains active and has evolved; current contracts and source govern its present meaning."
    }
    else {
        $disposition = Get-MissingDisposition -Path $entry.path
        $treatment = $disposition[0]
        $reason = $disposition[1]
    }

    [void]$recoveryEntries.Add([pscustomobject][ordered]@{
        path = $entry.path
        currentState = $state
        treatment = $treatment
        reason = $reason
    })
}

$stateCounts = [ordered]@{}
foreach ($group in ($inventoryEntries | Group-Object currentState | Sort-Object Name)) {
    $stateCounts[$group.Name] = $group.Count
}
$treatmentCounts = [ordered]@{}
foreach ($group in ($recoveryEntries | Group-Object treatment | Sort-Object Name)) {
    $treatmentCounts[$group.Name] = $group.Count
}

$inventory = [ordered]@{
    schemaVersion = 1
    sourceRevision = $Revision
    generatedBy = "scripts/harvest-0.8-artifacts.ps1"
    counts = [ordered]@{
        total = $inventoryEntries.Count
        byCurrentState = $stateCounts
    }
    entries = $inventoryEntries
}
$recovery = [ordered]@{
    schemaVersion = 1
    sourceRevision = $Revision
    generatedBy = "scripts/harvest-0.8-artifacts.ps1"
    counts = [ordered]@{
        total = $recoveryEntries.Count
        byTreatment = $treatmentCounts
    }
    entries = $recoveryEntries
}

foreach ($output in @(
    @{ Path = $InventoryPath; Document = $inventory },
    @{ Path = $RecoveryPath; Document = $recovery }
)) {
    $parent = Split-Path -Parent $output.Path
    if (-not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Path $parent | Out-Null
    }
    $json = $output.Document | ConvertTo-Json -Depth 8
    $resolvedOutput = if ([System.IO.Path]::IsPathRooted($output.Path)) {
        [System.IO.Path]::GetFullPath($output.Path)
    }
    else {
        [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $output.Path))
    }
    [System.IO.File]::WriteAllText(
        $resolvedOutput,
        $json + "`n",
        [System.Text.UTF8Encoding]::new($false)
    )
}

Write-Output "Inventoried $($inventoryEntries.Count) artifacts from $Revision."
foreach ($key in $stateCounts.Keys) {
    Write-Output "  $key`: $($stateCounts[$key])"
}
