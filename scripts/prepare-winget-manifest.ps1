# SPDX-License-Identifier: Apache-2.0 OR MIT
# Fill the WinGet manifest template from a verified release candidate and split it into the
# three files the community repository expects.

param(
    [Parameter(Mandatory = $true)]
    [string]$CandidateDirectory,
    [string]$OutputDirectory = "dist/winget-manifest",
    [switch]$Force,
    [switch]$ValidateWinGet
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$CandidateDirectory = [System.IO.Path]::GetFullPath($CandidateDirectory)
$candidate = Get-Content -LiteralPath (Join-Path $CandidateDirectory "release-candidate.json") -Raw | ConvertFrom-Json
& (Join-Path $PSScriptRoot "check-release-candidate.ps1") -CandidateDirectory $CandidateDirectory
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) {
    if (-not $Force) { throw "Output exists: $OutputDirectory (pass -Force to replace it)" }
    if ([System.IO.Path]::GetFileName($OutputDirectory) -ne "winget-manifest") {
        throw "Refusing to replace an output directory not named winget-manifest"
    }
    Remove-Item -LiteralPath $OutputDirectory -Recurse -Force
}

$archives = @($candidate.artifacts | Where-Object {
    $_.kind -eq "winget-archive" -and $_.target -eq "x86_64-pc-windows-msvc"
})
if ($archives.Count -ne 1) {
    throw "Candidate does not contain exactly one WinGet archive"
}
$hash = $archives[0].sha256
if ($hash -notmatch '^[0-9a-f]{64}$') { throw "Candidate has an invalid WinGet archive hash" }

$replacements = [ordered]@{
    "__VERSION__" = $candidate.version
    "__SHA_X86_64_PC_WINDOWS_MSVC_UPPER__" = $hash.ToUpperInvariant()
}
$content = Get-Content -LiteralPath (Join-Path $repo "packaging/winget/Sylin.Ghostlight.yaml") -Raw
foreach ($entry in $replacements.GetEnumerator()) {
    $content = $content.Replace($entry.Key, $entry.Value)
}
if ($content -match '__[A-Z0-9_]+__') { throw "Unresolved token in the WinGet template" }
$content = $content.Replace("`r`n", "`n")

$documents = @($content -split '(?m)^---\n')
if ($documents.Count -ne 3) { throw "WinGet template must contain exactly three documents" }
$files = @(
    "Sylin.Ghostlight.yaml",
    "Sylin.Ghostlight.installer.yaml",
    "Sylin.Ghostlight.locale.en-US.yaml"
)
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$utf8 = [System.Text.UTF8Encoding]::new($false)
for ($index = 0; $index -lt $files.Count; $index += 1) {
    [System.IO.File]::WriteAllText((Join-Path $OutputDirectory $files[$index]), $documents[$index], $utf8)
}
if ($ValidateWinGet) {
    $wingetCommand = Get-Command winget -ErrorAction SilentlyContinue
    if (-not $wingetCommand) { throw "WinGet validation was requested but winget is unavailable" }
    & winget validate --manifest $OutputDirectory
    if ($LASTEXITCODE -ne 0) { throw "winget validate rejected the prepared manifest" }
}
Write-Output "WinGet manifest prepared at $OutputDirectory"
