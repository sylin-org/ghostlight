# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [switch]$Json
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot

function Read-JsonFile {
    param([string]$Path)
    return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Assert-Semver {
    param([string]$Value, [string]$Label)
    if ($Value -notmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
        throw "$Label must be a stable semantic version, got '$Value'"
    }
}

function Test-Coverage {
    param($Row, [string]$ServiceVersion)

    if ($Row.PSObject.Properties.Name -contains "serviceVersionBlock") {
        return $ServiceVersion.StartsWith("$($Row.serviceVersionBlock).")
    }
    $service = [version]$ServiceVersion
    return $service -ge [version]$Row.minimumServiceVersion -and
        $service -le [version]$Row.maximumServiceVersion
}

$compatibilityPath = Join-Path $repo "compatibility.json"
$compatibility = Read-JsonFile -Path $compatibilityPath
if ($compatibility.schemaVersion -ne 2) {
    throw "compatibility.json must use schemaVersion 2"
}
if (@($compatibility.chromeAdapters).Count -eq 0) {
    throw "compatibility.json must declare at least one Chrome adapter"
}

$seenAdapters = [System.Collections.Generic.HashSet[string]]::new()
foreach ($row in $compatibility.chromeAdapters) {
    Assert-Semver -Value $row.adapterVersion -Label "adapterVersion"
    if (-not $seenAdapters.Add($row.adapterVersion)) {
        throw "duplicate adapterVersion $($row.adapterVersion)"
    }

    $hasBlock = $row.PSObject.Properties.Name -contains "serviceVersionBlock"
    $hasMinimum = $row.PSObject.Properties.Name -contains "minimumServiceVersion"
    $hasMaximum = $row.PSObject.Properties.Name -contains "maximumServiceVersion"
    if ($hasBlock -eq ($hasMinimum -or $hasMaximum)) {
        throw "adapter $($row.adapterVersion) must use exactly one block or one minimum/maximum range"
    }
    if ($hasBlock) {
        if ($row.serviceVersionBlock -notmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
            throw "adapter $($row.adapterVersion) has an invalid serviceVersionBlock"
        }
    }
    else {
        if (-not ($hasMinimum -and $hasMaximum)) {
            throw "adapter $($row.adapterVersion) must declare both range endpoints"
        }
        Assert-Semver -Value $row.minimumServiceVersion -Label "minimumServiceVersion"
        Assert-Semver -Value $row.maximumServiceVersion -Label "maximumServiceVersion"
        if ([version]$row.minimumServiceVersion -gt [version]$row.maximumServiceVersion) {
            throw "adapter $($row.adapterVersion) has an inverted service range"
        }
    }
}

$cargoText = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
$cargoMatch = [regex]::Match(
    $cargoText,
    '(?ms)^\[workspace\.package\].*?^version\s*=\s*"(?<version>[^"]+)"'
)
if (-not $cargoMatch.Success) {
    throw "Could not read workspace.package.version from Cargo.toml"
}
$serviceVersion = $cargoMatch.Groups["version"].Value
$tauriVersion = (Read-JsonFile -Path (Join-Path $repo "crates/orchestrator/tauri.conf.json")).version
$adapterVersion = (Read-JsonFile -Path (Join-Path $repo "extension/manifest.json")).version
foreach ($pair in @(
    @("workspace service", $serviceVersion),
    @("Tauri package", $tauriVersion),
    @("source adapter", $adapterVersion)
)) {
    Assert-Semver -Value $pair[1] -Label $pair[0]
}
if ($tauriVersion -ne $serviceVersion) {
    throw "Tauri package $tauriVersion does not match service $serviceVersion"
}

$sourceRow = @($compatibility.chromeAdapters | Where-Object {
    $_.adapterVersion -eq $adapterVersion
})
if ($sourceRow.Count -ne 1 -or -not (Test-Coverage -Row $sourceRow[0] -ServiceVersion $serviceVersion)) {
    throw "source adapter $adapterVersion does not cover source service $serviceVersion"
}

$publicStatus = Read-JsonFile -Path (Join-Path $repo "docs/public-status.json")
$publicAdapter = $publicStatus.chromeStore.publicAdapterVersion
$publicService = $publicStatus.release
Assert-Semver -Value $publicAdapter -Label "public adapter"
Assert-Semver -Value $publicService -Label "public service"
$publicRow = @($compatibility.chromeAdapters | Where-Object {
    $_.adapterVersion -eq $publicAdapter
})
if ($publicRow.Count -ne 1 -or -not (Test-Coverage -Row $publicRow[0] -ServiceVersion $publicService)) {
    throw "public adapter $publicAdapter does not cover public service $publicService"
}

$result = [ordered]@{
    schemaVersion = $compatibility.schemaVersion
    source = [ordered]@{
        service = $serviceVersion
        desktopPackage = $tauriVersion
        chromeAdapter = $adapterVersion
        compatible = $true
    }
    public = [ordered]@{
        service = $publicService
        chromeAdapter = $publicAdapter
        compatible = $true
    }
    declaredAdapters = @($compatibility.chromeAdapters).Count
}

if ($Json) {
    $result | ConvertTo-Json -Depth 5
}
else {
    Write-Output "Source: Ghostlight $serviceVersion with Chrome adapter $adapterVersion -- compatible"
    Write-Output "Public: Ghostlight $publicService with Chrome adapter $publicAdapter -- compatible"
    Write-Output "Declared adapter rows: $(@($compatibility.chromeAdapters).Count)"
}
