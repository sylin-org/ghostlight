# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$CandidateDirectory
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$CandidateDirectory = [System.IO.Path]::GetFullPath($CandidateDirectory)
$assetDirectory = Join-Path $CandidateDirectory "assets"
$manifestPath = Join-Path $CandidateDirectory "release-candidate.json"
$sumsPath = Join-Path $CandidateDirectory "SHA256SUMS"
foreach ($path in @($assetDirectory, $manifestPath, $sumsPath)) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Candidate is missing required path: $path"
    }
}

$candidate = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($candidate.schemaVersion -ne 1 -or
    $candidate.generatedBy -ne "scripts/assemble-release-candidate.ps1" -or
    $candidate.status -ne "release-candidate") {
    throw "Candidate manifest metadata is invalid"
}
if ($candidate.version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$' -or
    $candidate.sourceRevision -notmatch '^[0-9a-f]{40,64}$') {
    throw "Candidate version or source revision is invalid"
}

$fixedCoordinates = @(
    "browser-adapter|chromium-store",
    "client-bundle|claude-desktop",
    "client-launcher|npm",
    "raw-binary|ghostlight@x86_64-pc-windows-msvc",
    "raw-binary|ghostlight@x86_64-unknown-linux-gnu",
    "raw-binary|ghostlight-desktop@x86_64-pc-windows-msvc",
    "raw-binary|ghostlight-desktop@x86_64-unknown-linux-gnu",
    "raw-binary|ghostlight-service@x86_64-pc-windows-msvc",
    "raw-binary|ghostlight-service@x86_64-unknown-linux-gnu",
    "winget-archive|x86_64-pc-windows-msvc"
)

# Read the workspace member package names at the candidate's source revision.
# Returns $null only when that revision is not available locally. Once the
# revision exists, anything unreadable or unexpected in the manifests throws.
function Get-WorkspacePackageNames([string]$revision) {
    $repo = Split-Path -Parent $PSScriptRoot
    & git -C $repo cat-file -e "${revision}^{commit}" 2>$null
    if ($LASTEXITCODE -ne 0) { return $null }
    $root = & git -C $repo show "${revision}:Cargo.toml" 2>$null
    if ($LASTEXITCODE -ne 0) { throw "Cargo.toml is unreadable at source revision $revision" }
    $block = [regex]::Match(($root -join "`n"), '(?ms)^\[workspace\].*?^members\s*=\s*\[(?<members>.*?)\]')
    if (-not $block.Success) { throw "Cargo.toml at source revision $revision has no workspace members block" }
    $names = [System.Collections.Generic.List[string]]::new()
    foreach ($member in [regex]::Matches($block.Groups["members"].Value, '"([^"]+)"')) {
        $path = $member.Groups[1].Value
        if ($path -match '[*?\[]') { throw "Workspace member is a glob and cannot be verified: $path" }
        $manifest = & git -C $repo show "${revision}:$path/Cargo.toml" 2>$null
        if ($LASTEXITCODE -ne 0) { throw "Workspace member $path has no Cargo.toml at source revision $revision" }
        $name = [regex]::Match(($manifest -join "`n"), '(?ms)^\[package\].*?^name\s*=\s*"(?<name>[^"]+)"')
        if (-not $name.Success) { throw "Workspace member $path has no package name at source revision $revision" }
        [void]$names.Add($name.Groups["name"].Value)
    }
    if ($names.Count -eq 0) { throw "Cargo.toml at source revision $revision lists no workspace members" }
    return $names.ToArray()
}

$artifacts = @($candidate.artifacts | Sort-Object name)
if ($artifacts.Count -eq 0) {
    throw "Candidate contains no artifacts"
}
$coordinates = @($artifacts | ForEach-Object { "$($_.kind)|$($_.target)" } | Sort-Object)
if (@($coordinates | Select-Object -Unique).Count -ne $coordinates.Count) {
    throw "Candidate artifact coordinates are duplicated"
}
$sbomNames = Get-WorkspacePackageNames $candidate.sourceRevision
if ($null -ne $sbomNames) {
    $expectedCoordinates = @($fixedCoordinates + @($sbomNames | ForEach-Object { "sbom|$_" }))
    if (($coordinates -join "`n") -ne (($expectedCoordinates | Sort-Object) -join "`n")) {
        throw "Candidate artifact coordinates are incomplete or unexpected"
    }
} else {
    Write-Warning "Source revision $($candidate.sourceRevision) is not available locally; SBOM membership was not verified."
    $sboms = @($artifacts | Where-Object { $_.kind -eq "sbom" })
    if ($sboms.Count -eq 0) {
        throw "Candidate contains no SBOM artifacts"
    }
    foreach ($sbom in $sboms) {
        if ($sbom.name -ne "ghostlight-v$($candidate.version)-sbom-$($sbom.target).cyclonedx.json") {
            throw "SBOM target does not match its asset name: $($sbom.name)"
        }
    }
    $others = @($coordinates | Where-Object { $_ -notlike "sbom|*" })
    if (($others -join "`n") -ne (($fixedCoordinates | Sort-Object) -join "`n")) {
        throw "Candidate artifact coordinates are incomplete or unexpected"
    }
}

$actualNames = @(Get-ChildItem -LiteralPath $assetDirectory -File | Select-Object -ExpandProperty Name | Sort-Object)
$manifestNames = @($artifacts | Select-Object -ExpandProperty name | Sort-Object)
if (($actualNames -join "`n") -ne ($manifestNames -join "`n")) {
    throw "Candidate assets and manifest entries differ"
}

$sumLines = [System.Collections.Generic.List[string]]::new()
foreach ($artifact in $artifacts) {
    if ([System.IO.Path]::GetFileName($artifact.name) -ne $artifact.name) {
        throw "Candidate artifact name is not a basename: $($artifact.name)"
    }
    $path = Join-Path $assetDirectory $artifact.name
    $item = Get-Item -LiteralPath $path
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($item.Length -ne $artifact.bytes -or $hash -ne $artifact.sha256) {
        throw "Candidate artifact does not match its manifest: $($artifact.name)"
    }
    [void]$sumLines.Add("$hash  $($artifact.name)")
}

$expectedSums = ($sumLines -join "`n") + "`n"
$actualSums = [System.IO.File]::ReadAllText($sumsPath).Replace("`r`n", "`n")
if ($actualSums -ne $expectedSums) {
    throw "SHA256SUMS is not the exact sorted candidate manifest"
}

Write-Output "Release candidate verified: version $($candidate.version), source $($candidate.sourceRevision), $($artifacts.Count) artifacts."
exit 0
