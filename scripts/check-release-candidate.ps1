# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$CandidateDirectory,
    [ValidateSet("unsigned-build-candidate", "signed-release-candidate")]
    [string]$ExpectedStatus = "unsigned-build-candidate"
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
    $candidate.status -ne $ExpectedStatus) {
    throw "Candidate manifest metadata is invalid"
}
if ($candidate.version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$' -or
    $candidate.sourceRevision -notmatch '^[0-9a-f]{40,64}$') {
    throw "Candidate version or source revision is invalid"
}

$expectedCoordinates = @(
    "browser-adapter|chromium-store",
    "client-bundle|claude-desktop",
    "client-launcher|npm",
    "native-package|aarch64-apple-darwin",
    "native-package|x86_64-apple-darwin",
    "native-package|x86_64-pc-windows-msvc",
    "native-package|x86_64-unknown-linux-gnu",
    "portable-package|aarch64-apple-darwin",
    "portable-package|x86_64-apple-darwin",
    "portable-package|x86_64-pc-windows-msvc",
    "portable-package|x86_64-unknown-linux-gnu",
    "raw-binary|ghostlight@aarch64-apple-darwin",
    "raw-binary|ghostlight@x86_64-apple-darwin",
    "raw-binary|ghostlight@x86_64-pc-windows-msvc",
    "raw-binary|ghostlight@x86_64-unknown-linux-gnu",
    "raw-binary|ghostlight-browser-connector@aarch64-apple-darwin",
    "raw-binary|ghostlight-browser-connector@x86_64-apple-darwin",
    "raw-binary|ghostlight-browser-connector@x86_64-pc-windows-msvc",
    "raw-binary|ghostlight-browser-connector@x86_64-unknown-linux-gnu",
    "raw-binary|ghostlight-mcp-connector@aarch64-apple-darwin",
    "raw-binary|ghostlight-mcp-connector@x86_64-apple-darwin",
    "raw-binary|ghostlight-mcp-connector@x86_64-pc-windows-msvc",
    "raw-binary|ghostlight-mcp-connector@x86_64-unknown-linux-gnu",
    "sbom|ghostlight",
    "sbom|ghostlight-bridge",
    "sbom|ghostlight-browser-connector",
    "sbom|ghostlight-mcp-connector"
)
$artifacts = @($candidate.artifacts | Sort-Object name)
if ($artifacts.Count -ne $expectedCoordinates.Count) {
    throw "Candidate must contain exactly $($expectedCoordinates.Count) artifacts"
}
$coordinates = @($artifacts | ForEach-Object { "$($_.kind)|$($_.target)" } | Sort-Object)
if (($coordinates -join "`n") -ne (($expectedCoordinates | Sort-Object) -join "`n")) {
    throw "Candidate artifact coordinates are incomplete or duplicated"
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
