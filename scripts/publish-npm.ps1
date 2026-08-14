# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$CandidateDirectory,
    [Parameter(Mandatory = $true)]
    [string]$Package,
    [ValidateSet("Plan", "Publish")]
    [string]$Mode = "Plan",
    [string]$Repository = "sylin-org/ghostlight",
    [string]$ExpectedSha256,
    [switch]$Execute
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$CandidateDirectory = [System.IO.Path]::GetFullPath($CandidateDirectory)
$Package = [System.IO.Path]::GetFullPath($Package)
if ($Repository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') {
    throw "Repository must have owner/name form"
}
& (Join-Path $PSScriptRoot "check-release-candidate.ps1") -CandidateDirectory $CandidateDirectory
$candidate = Get-Content -LiteralPath (Join-Path $CandidateDirectory "release-candidate.json") -Raw | ConvertFrom-Json
if ([System.IO.Path]::GetFileName($Package) -ne "ghostlight-$($candidate.version).tgz") {
    throw "npm tarball name does not match release candidate version"
}
$observedHash = (Get-FileHash -LiteralPath $Package -Algorithm SHA256).Hash.ToLowerInvariant()
$packageArtifacts = @($candidate.artifacts | Where-Object {
    $_.kind -eq "client-launcher" -and $_.target -eq "npm"
})
if ($packageArtifacts.Count -ne 1 -or
    $packageArtifacts[0].name -ne [System.IO.Path]::GetFileName($Package) -or
    $packageArtifacts[0].sha256 -ne $observedHash) {
    throw "npm tarball is not the exact launcher bound into the release candidate"
}

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
$tag = "v$($candidate.version)"
& gh release view $tag --repo $Repository *> $null
if ($LASTEXITCODE -ne 0) {
    throw "GitHub release $tag does not exist; refusing to publish npm before its assets"
}
& gh attestation verify $Package `
    --repo $Repository `
    --signer-workflow "$Repository/.github/workflows/release.yml" `
    --source-digest $candidate.sourceRevision `
    --deny-self-hosted-runners *> $null
if ($LASTEXITCODE -ne 0) {
    throw "GitHub provenance verification failed for the npm tarball"
}
& npm publish $Package --access public
if ($LASTEXITCODE -ne 0) {
    throw "npm publish failed"
}
