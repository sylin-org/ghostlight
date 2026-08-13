# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$CandidateDirectory,
    [string]$OutputDirectory = "dist/npm-package",
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$CandidateDirectory = [System.IO.Path]::GetFullPath($CandidateDirectory)
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
& (Join-Path $PSScriptRoot "check-release-candidate.ps1") -CandidateDirectory $CandidateDirectory

$candidate = Get-Content -LiteralPath (Join-Path $CandidateDirectory "release-candidate.json") -Raw | ConvertFrom-Json
$packageSource = Join-Path $repo "packaging/npm"
$packageJson = Get-Content -LiteralPath (Join-Path $packageSource "package.json") -Raw | ConvertFrom-Json
if ($packageJson.name -ne "ghostlight" -or $packageJson.version -ne $candidate.version) {
    throw "npm package identity does not match release candidate version $($candidate.version)"
}

$rawBinaries = @($candidate.artifacts | Where-Object kind -eq "raw-binary" | Sort-Object name)
if ($rawBinaries.Count -ne 12) {
    throw "npm preparation requires exactly 12 raw binaries, found $($rawBinaries.Count)"
}
$checksums = [ordered]@{
    version = $candidate.version
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
foreach ($legalFile in @("LICENSE-APACHE", "LICENSE-MIT", "LICENSE-COMMERCIAL", "LICENSING.md")) {
    Copy-Item -LiteralPath (Join-Path $repo $legalFile) -Destination $stagedPackage
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
$tarballs = @(Get-ChildItem -LiteralPath $OutputDirectory -File -Filter "ghostlight-$($candidate.version).tgz")
if ($tarballs.Count -ne 1) {
    throw "npm pack did not produce the exact Ghostlight tarball"
}
Write-Output "npm package prepared: $($tarballs[0].FullName)"
