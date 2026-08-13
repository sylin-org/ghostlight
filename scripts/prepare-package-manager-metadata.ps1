# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$CandidateDirectory,
    [string]$OutputDirectory = "dist/package-manager-metadata",
    [switch]$Force,
    [switch]$ValidateWinGet
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$CandidateDirectory = [System.IO.Path]::GetFullPath($CandidateDirectory)
$candidate = Get-Content -LiteralPath (Join-Path $CandidateDirectory "release-candidate.json") -Raw | ConvertFrom-Json
& (Join-Path $PSScriptRoot "check-release-candidate.ps1") `
    -CandidateDirectory $CandidateDirectory `
    -ExpectedStatus $candidate.status
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) {
    if (-not $Force) { throw "Output exists: $OutputDirectory (pass -Force to replace it)" }
    if ([System.IO.Path]::GetFileName($OutputDirectory) -ne "package-manager-metadata") {
        throw "Refusing to replace an output directory not named package-manager-metadata"
    }
    Remove-Item -LiteralPath $OutputDirectory -Recurse -Force
}

function Portable-Hash([string]$Target) {
    $portableArtifacts = @($candidate.artifacts | Where-Object {
        $_.kind -eq "portable-package" -and $_.target -eq $Target
    })
    if ($portableArtifacts.Count -ne 1) {
        throw "Candidate does not contain exactly one portable package for $Target"
    }
    $hash = $portableArtifacts[0].sha256
    if ($hash -notmatch '^[0-9a-f]{64}$') { throw "Candidate has an invalid hash for $Target" }
    return $hash
}

$replacements = [ordered]@{
    "__VERSION__" = $candidate.version
    "__SHA_AARCH64_APPLE_DARWIN__" = Portable-Hash "aarch64-apple-darwin"
    "__SHA_X86_64_APPLE_DARWIN__" = Portable-Hash "x86_64-apple-darwin"
    "__SHA_X86_64_UNKNOWN_LINUX_GNU__" = Portable-Hash "x86_64-unknown-linux-gnu"
    "__SHA_X86_64_PC_WINDOWS_MSVC__" = Portable-Hash "x86_64-pc-windows-msvc"
}
$replacements["__SHA_X86_64_PC_WINDOWS_MSVC_UPPER__"] = $replacements["__SHA_X86_64_PC_WINDOWS_MSVC__"].ToUpperInvariant()

function Expand-Template([string]$RelativePath) {
    $content = Get-Content -LiteralPath (Join-Path $repo $RelativePath) -Raw
    foreach ($entry in $replacements.GetEnumerator()) {
        $content = $content.Replace($entry.Key, $entry.Value)
    }
    if ($content -match '__[A-Z0-9_]+__') { throw "Unresolved token in $RelativePath" }
    return $content.Replace("`r`n", "`n")
}

$homebrewDirectory = Join-Path $OutputDirectory "homebrew"
$scoopDirectory = Join-Path $OutputDirectory "scoop"
$wingetDirectory = Join-Path $OutputDirectory "winget"
foreach ($directory in @($homebrewDirectory, $scoopDirectory, $wingetDirectory)) {
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
}
$utf8 = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText(
    (Join-Path $homebrewDirectory "ghostlight.rb"),
    (Expand-Template "packaging/homebrew/ghostlight.rb"),
    $utf8
)
$scoop = Expand-Template "packaging/scoop/ghostlight.json"
[void]($scoop | ConvertFrom-Json)
[System.IO.File]::WriteAllText((Join-Path $scoopDirectory "ghostlight.json"), $scoop, $utf8)

$winget = Expand-Template "packaging/winget/Sylin.Ghostlight.yaml"
$documents = @($winget -split '(?m)^---\n')
if ($documents.Count -ne 3) { throw "WinGet template must contain exactly three documents" }
$wingetFiles = @(
    "Sylin.Ghostlight.yaml",
    "Sylin.Ghostlight.installer.yaml",
    "Sylin.Ghostlight.locale.en-US.yaml"
)
for ($index = 0; $index -lt $wingetFiles.Count; $index += 1) {
    [System.IO.File]::WriteAllText(
        (Join-Path $wingetDirectory $wingetFiles[$index]),
        $documents[$index],
        $utf8
    )
}
if ($ValidateWinGet) {
    $wingetCommand = Get-Command winget -ErrorAction SilentlyContinue
    if (-not $wingetCommand) { throw "WinGet validation was requested but winget is unavailable" }
    & winget validate --manifest $wingetDirectory
    if ($LASTEXITCODE -ne 0) { throw "winget validate rejected the prepared manifests" }
}
Write-Output "Package-manager metadata prepared at $OutputDirectory"
