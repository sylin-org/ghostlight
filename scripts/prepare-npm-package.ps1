# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$CandidateDirectory,
    [string]$ArtifactRoot,
    [string]$Version,
    [string]$OutputDirectory = "dist/npm-package",
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
if ([string]::IsNullOrWhiteSpace($CandidateDirectory) -eq [string]::IsNullOrWhiteSpace($ArtifactRoot)) {
    throw "Provide exactly one of -CandidateDirectory or -ArtifactRoot"
}
$rawBinaries = @()
if (-not [string]::IsNullOrWhiteSpace($CandidateDirectory)) {
    $CandidateDirectory = [System.IO.Path]::GetFullPath($CandidateDirectory)
    $candidate = Get-Content -LiteralPath (Join-Path $CandidateDirectory "release-candidate.json") -Raw | ConvertFrom-Json
    & (Join-Path $PSScriptRoot "check-release-candidate.ps1") `
        -CandidateDirectory $CandidateDirectory `
        -ExpectedStatus $candidate.status
    $Version = $candidate.version
    $rawBinaries = @($candidate.artifacts | Where-Object kind -eq "raw-binary" | Sort-Object name)
} else {
    $ArtifactRoot = [System.IO.Path]::GetFullPath($ArtifactRoot)
    if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
        throw "Artifact preparation requires a three-part -Version"
    }
    foreach ($target in @(
        [ordered]@{ name = "x86_64-pc-windows-msvc"; extension = ".exe" },
        [ordered]@{ name = "x86_64-unknown-linux-gnu"; extension = "" },
        [ordered]@{ name = "aarch64-apple-darwin"; extension = "" },
        [ordered]@{ name = "x86_64-apple-darwin"; extension = "" }
    )) {
        foreach ($component in @("ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector")) {
            $name = "$component-$($target.name)$($target.extension)"
            $found = @(Get-ChildItem -LiteralPath $ArtifactRoot -Recurse -File -Filter $name)
            if ($found.Count -ne 1) { throw "Expected one raw artifact $name, found $($found.Count)" }
            $rawBinaries += [pscustomobject]@{
                name = $name
                sha256 = (Get-FileHash -LiteralPath $found[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            }
        }
    }
    $rawBinaries = @($rawBinaries | Sort-Object name)
}
$packageSource = Join-Path $repo "packaging/npm"
$packageJson = Get-Content -LiteralPath (Join-Path $packageSource "package.json") -Raw | ConvertFrom-Json
if ($packageJson.name -ne "ghostlight" -or $packageJson.version -ne $Version) {
    throw "npm package identity does not match release version $Version"
}

if ($rawBinaries.Count -ne 12) {
    throw "npm preparation requires exactly 12 raw binaries, found $($rawBinaries.Count)"
}
$checksums = [ordered]@{
    version = $Version
    algorithm = "sha256"
    binaries = [ordered]@{}
}
foreach ($binary in $rawBinaries) {
    $checksums.binaries[$binary.name] = $binary.sha256
}

if (Test-Path -LiteralPath $OutputDirectory) {
    if (-not $Force) {
        throw "Output directory already exists: $OutputDirectory (pass -Force to replace it)"
    }
    if ([System.IO.Path]::GetFileName($OutputDirectory) -ne "npm-package") {
        throw "Refusing to replace an output directory not named npm-package: $OutputDirectory"
    }
    Remove-Item -LiteralPath $OutputDirectory -Recurse -Force
}
$stagedPackage = Join-Path $OutputDirectory "package"
New-Item -ItemType Directory -Path $stagedPackage -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $packageSource "package.json") -Destination $stagedPackage
Copy-Item -LiteralPath (Join-Path $packageSource "README.md") -Destination $stagedPackage
Copy-Item -LiteralPath (Join-Path $packageSource "bin") -Destination $stagedPackage -Recurse
Copy-Item -LiteralPath (Join-Path $packageSource "test") -Destination $stagedPackage -Recurse
foreach ($legalFile in @(
    [ordered]@{ source = "LICENSE"; destination = "LICENSE" },
    [ordered]@{ source = "docs/licenses/MIT.txt"; destination = "MIT.txt" },
    [ordered]@{ source = "docs/licenses/LicenseRef-Ghostlight-Commercial.txt"; destination = "LicenseRef-Ghostlight-Commercial.txt" },
    [ordered]@{ source = "LICENSING.md"; destination = "LICENSING.md" }
)) {
    Copy-Item -LiteralPath (Join-Path $repo $legalFile.source) -Destination (Join-Path $stagedPackage $legalFile.destination)
}
[System.IO.File]::WriteAllText(
    (Join-Path $stagedPackage "checksums.json"),
    ($checksums | ConvertTo-Json -Depth 4) + "`n",
    [System.Text.UTF8Encoding]::new($false)
)

& npm test --prefix $stagedPackage
if ($LASTEXITCODE -ne 0) {
    throw "Staged npm launcher tests failed"
}
& npm pack $stagedPackage --pack-destination $OutputDirectory
if ($LASTEXITCODE -ne 0) {
    throw "npm pack failed"
}
$tarballs = @(Get-ChildItem -LiteralPath $OutputDirectory -File -Filter "ghostlight-$Version.tgz")
if ($tarballs.Count -ne 1) {
    throw "npm pack did not produce the exact Ghostlight tarball"
}
Write-Output "npm package prepared: $($tarballs[0].FullName)"
