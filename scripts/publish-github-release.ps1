#Requires -Version 7
# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$CandidateDirectory,
    [ValidateSet("Plan", "CreateDraft", "PublishDraft")]
    [string]$Action = "Plan",
    [string]$Repository = "sylin-org/ghostlight",
    [string]$NotesFile,
    [switch]$Execute
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ($Action -ne "Plan" -and -not $Execute) {
    throw "$Action changes GitHub release state. Pass -Execute only after owner approval for that exact action."
}
if ($Repository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') {
    throw "Repository must have owner/name form"
}

$CandidateDirectory = [System.IO.Path]::GetFullPath($CandidateDirectory)
$candidatePath = Join-Path $CandidateDirectory "release-candidate.json"
if (-not (Test-Path -LiteralPath $candidatePath -PathType Leaf)) {
    throw "Candidate manifest does not exist: $candidatePath"
}
$candidate = Get-Content -LiteralPath $candidatePath -Raw | ConvertFrom-Json
$allowedStatuses = @("unsigned-build-candidate", "signed-release-candidate")
if ($allowedStatuses -notcontains $candidate.status) {
    throw "Unknown candidate status: $($candidate.status)"
}
& (Join-Path $PSScriptRoot "check-release-candidate.ps1") `
    -CandidateDirectory $CandidateDirectory `
    -ExpectedStatus $candidate.status

$tag = "v$($candidate.version)"
$releaseFiles = @(
    Get-ChildItem -LiteralPath (Join-Path $CandidateDirectory "assets") -File |
        Sort-Object Name |
        Select-Object -ExpandProperty FullName
)
$releaseFiles += @(
    (Join-Path $CandidateDirectory "release-candidate.json"),
    (Join-Path $CandidateDirectory "SHA256SUMS")
)

Write-Output "GitHub release action: $Action"
Write-Output "Repository: $Repository"
Write-Output "Tag: $tag"
Write-Output "Source revision: $($candidate.sourceRevision)"
Write-Output "Candidate status: $($candidate.status)"
Write-Output "Release files: $($releaseFiles.Count)"
if ($Action -eq "Plan") {
    if ($candidate.status -ne "signed-release-candidate") {
        Write-Output "Publication blocker: native candidates are not signed."
    }
    Write-Output "No GitHub request was made."
    return
}
if ($candidate.status -ne "signed-release-candidate") {
    throw "GitHub publication refuses an unsigned build candidate"
}

$remoteRevision = (& gh api "repos/$Repository/commits/$tag" --jq .sha).Trim()
if ($LASTEXITCODE -ne 0 -or $remoteRevision -ne $candidate.sourceRevision) {
    throw "Remote tag $tag does not resolve to the candidate source revision"
}
$signerWorkflow = "$Repository/.github/workflows/release.yml"
foreach ($path in $releaseFiles) {
    & gh attestation verify $path `
        --repo $Repository `
        --signer-workflow $signerWorkflow `
        --source-digest $candidate.sourceRevision `
        --deny-self-hosted-runners *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "GitHub provenance verification failed: $([System.IO.Path]::GetFileName($path))"
    }
}

if ($Action -eq "CreateDraft") {
    if ([string]::IsNullOrWhiteSpace($NotesFile)) {
        throw "CreateDraft requires -NotesFile"
    }
    $NotesFile = [System.IO.Path]::GetFullPath($NotesFile)
    if (-not (Test-Path -LiteralPath $NotesFile -PathType Leaf)) {
        throw "Release notes do not exist: $NotesFile"
    }
    & gh release view $tag --repo $Repository *> $null
    if ($LASTEXITCODE -eq 0) {
        throw "GitHub release already exists: $tag"
    }
    $arguments = @(
        "release", "create", $tag,
        "--repo", $Repository,
        "--title", "Ghostlight $($candidate.version)",
        "--notes-file", $NotesFile,
        "--verify-tag",
        "--draft"
    ) + $releaseFiles
    & gh @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "GitHub draft release creation failed"
    }
    Write-Output "Created GitHub draft release: $tag"
    return
}

$release = & gh release view $tag --repo $Repository --json isDraft,tagName | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or -not $release.isDraft -or $release.tagName -ne $tag) {
    throw "GitHub release is missing or is not a draft: $tag"
}

$downloadRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("ghostlight-release-" + [guid]::NewGuid())
[System.IO.Directory]::CreateDirectory($downloadRoot) | Out-Null
try {
    & gh release download $tag --repo $Repository --dir $downloadRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Could not download the draft release assets"
    }
    $expectedNames = @($releaseFiles | ForEach-Object { [System.IO.Path]::GetFileName($_) } | Sort-Object)
    $actualNames = @(Get-ChildItem -LiteralPath $downloadRoot -File | Select-Object -ExpandProperty Name | Sort-Object)
    if (($expectedNames -join "`n") -ne ($actualNames -join "`n")) {
        throw "Draft release asset names differ from the candidate"
    }
    foreach ($localPath in $releaseFiles) {
        $name = [System.IO.Path]::GetFileName($localPath)
        $remotePath = Join-Path $downloadRoot $name
        $localHash = (Get-FileHash -LiteralPath $localPath -Algorithm SHA256).Hash
        $remoteHash = (Get-FileHash -LiteralPath $remotePath -Algorithm SHA256).Hash
        if ($localHash -ne $remoteHash) {
            throw "Draft release asset differs from the candidate: $name"
        }
    }
}
finally {
    if ((Split-Path -Parent $downloadRoot) -eq [System.IO.Path]::GetTempPath().TrimEnd('\', '/')) {
        Remove-Item -LiteralPath $downloadRoot -Recurse -Force
    }
}

& gh release edit $tag --repo $Repository --draft=false
if ($LASTEXITCODE -ne 0) {
    throw "GitHub draft publication failed"
}
Write-Output "Published GitHub release: $tag"
