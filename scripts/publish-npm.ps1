# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$CandidateDirectory,
    [Parameter(Mandatory = $true)]
    [string]$Package,
    [ValidateSet("Plan", "Publish")]
    [string]$Mode = "Plan",
    [string]$ExpectedSha256,
    [switch]$Execute
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$CandidateDirectory = [System.IO.Path]::GetFullPath($CandidateDirectory)
$Package = [System.IO.Path]::GetFullPath($Package)
& (Join-Path $PSScriptRoot "check-release-candidate.ps1") -CandidateDirectory $CandidateDirectory -ExpectedStatus signed-release-candidate
$candidate = Get-Content -LiteralPath (Join-Path $CandidateDirectory "release-candidate.json") -Raw | ConvertFrom-Json
if ([System.IO.Path]::GetFileName($Package) -ne "ghostlight-$($candidate.version).tgz") {
    throw "npm tarball name does not match release candidate version"
}
$observedHash = (Get-FileHash -LiteralPath $Package -Algorithm SHA256).Hash.ToLowerInvariant()

Write-Output "npm publication plan"
Write-Output "  package: ghostlight@$($candidate.version)"
Write-Output "  source: $($candidate.sourceRevision)"
Write-Output "  tarball: $Package"
Write-Output "  sha256: $observedHash"
if ($Mode -eq "Plan") {
    Write-Output "Plan only; npm was not changed."
    exit 0
}
if (-not $Execute) {
    throw "Publish mode requires -Execute"
}
if ($ExpectedSha256 -notmatch '^[0-9a-fA-F]{64}$' -or
    $ExpectedSha256.ToLowerInvariant() -ne $observedHash) {
    throw "Publish mode requires the exact tarball SHA-256"
}
& npm publish $Package --access public
if ($LASTEXITCODE -ne 0) {
    throw "npm publish failed"
}
