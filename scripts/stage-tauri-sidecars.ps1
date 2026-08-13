# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$TargetTriple,
    [ValidateSet("debug", "release")]
    [string]$Profile = "release",
    [string]$TargetDirectory,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ($TargetTriple -notmatch '^[a-zA-Z0-9_.-]+$') {
    throw "TargetTriple contains unsupported characters: $TargetTriple"
}

$repo = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($TargetDirectory)) {
    $TargetDirectory = Join-Path $repo "target"
}
$TargetDirectory = [System.IO.Path]::GetFullPath($TargetDirectory)
$sourceDirectory = Join-Path $TargetDirectory "$TargetTriple/$Profile"
$stageDirectory = Join-Path $repo "crates/orchestrator/binaries"
$windows = $TargetTriple -match 'windows'
$suffix = if ($windows) { ".exe" } else { "" }
$names = @("ghostlight-mcp-connector", "ghostlight-browser-connector")

$sources = @{}
foreach ($name in $names) {
    $source = Join-Path $sourceDirectory "$name$suffix"
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "Required release sibling is missing: $source"
    }
    $sources[$name] = $source
}

if (-not (Test-Path -LiteralPath $stageDirectory)) {
    New-Item -ItemType Directory -Path $stageDirectory | Out-Null
}
foreach ($name in $names) {
    $destination = Join-Path $stageDirectory "$name-$TargetTriple$suffix"
    if ((Test-Path -LiteralPath $destination) -and -not $Force) {
        throw "Staged sidecar already exists: $destination (pass -Force to replace it)"
    }
    Copy-Item -LiteralPath $sources[$name] -Destination $destination -Force:$Force
    $sourceHash = (Get-FileHash -LiteralPath $sources[$name] -Algorithm SHA256).Hash
    $stagedHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
    if ($sourceHash -ne $stagedHash) {
        throw "Staged sidecar differs from its locked workspace build: $name"
    }
    Write-Output "Staged $name for $TargetTriple ($($stagedHash.ToLowerInvariant()))"
}

if ($TargetTriple -match 'linux') {
    $manifestDirectory = Join-Path $stageDirectory "native-host"
    if (-not (Test-Path -LiteralPath $manifestDirectory)) {
        New-Item -ItemType Directory -Path $manifestDirectory | Out-Null
    }
    $manifestPath = Join-Path $manifestDirectory "org.sylin.ghostlight.json"
    $manifest = [ordered]@{
        name = "org.sylin.ghostlight"
        description = "Ghostlight browser connector"
        path = "/usr/bin/ghostlight-browser-connector"
        type = "stdio"
        allowed_origins = @(
            "chrome-extension://lejccfmoeogmhemakeknjjdhkfkgncdl/",
            "chrome-extension://cjcmhepmagomefjggkcohdbfemacojoa/"
        )
    }
    $json = $manifest | ConvertTo-Json -Depth 5
    [System.IO.File]::WriteAllText(
        $manifestPath,
        $json + "`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output "Staged Linux system native-host manifest: $manifestPath"
}
