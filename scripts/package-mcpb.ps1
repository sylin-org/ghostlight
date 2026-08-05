<#
.SYNOPSIS
  Packages the Ghostlight Claude Desktop MCPB from raw release binaries.

.DESCRIPTION
  Stages the tracked MCPB manifest and launcher with Windows x64, macOS Apple Silicon, and macOS
  Intel release binaries. The resulting zip-format .mcpb is self-contained and performs no
  runtime download.

.PARAMETER Version
  The service version. It must match packaging/mcpb/manifest.json.

.PARAMETER ArtifactsDir
  Directory containing the raw per-target release binaries emitted by release.yml.
#>
param(
  [Parameter(Mandatory = $true)]
  [string]$Version,
  [Parameter(Mandatory = $true)]
  [string]$ArtifactsDir
)

$ErrorActionPreference = 'Stop'

if ($Version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') {
  throw "Version must be a semantic version, got: $Version"
}

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$SourceDir = Join-Path $RepoRoot 'packaging/mcpb'
$ManifestPath = Join-Path $SourceDir 'manifest.json'
$ArtifactsDir = (Resolve-Path $ArtifactsDir).Path
$DistDir = Join-Path $RepoRoot 'dist'
$TempRoot = (Resolve-Path ([System.IO.Path]::GetTempPath())).Path.TrimEnd([char[]]"\/")
$StageDir = Join-Path $TempRoot "ghostlight-mcpb-stage-$([guid]::NewGuid())"
$PackagePath = Join-Path $DistDir "ghostlight-v$Version.mcpb"
$ZipPath = Join-Path $DistDir "ghostlight-v$Version.zip"

$manifest = Get-Content $ManifestPath -Raw | ConvertFrom-Json
if ($manifest.version -ne $Version) {
  throw "Requested service version $Version does not match MCPB manifest version $($manifest.version)."
}

$targets = @(
  @{ Target = 'x86_64-pc-windows-msvc'; Suffix = '.exe' },
  @{ Target = 'aarch64-apple-darwin'; Suffix = '' },
  @{ Target = 'x86_64-apple-darwin'; Suffix = '' }
)

try {
  New-Item -ItemType Directory -Path (Join-Path $StageDir 'server') -Force | Out-Null
  Copy-Item -LiteralPath $ManifestPath -Destination (Join-Path $StageDir 'manifest.json')
  Copy-Item -LiteralPath (Join-Path $SourceDir 'README.md') -Destination (Join-Path $StageDir 'README.md')
  Copy-Item -LiteralPath (Join-Path $SourceDir 'server/launch.js') -Destination (Join-Path $StageDir 'server/launch.js')
  Copy-Item -LiteralPath (Join-Path $SourceDir 'icon.png') -Destination (Join-Path $StageDir 'icon.png')

  foreach ($target in $targets) {
    $targetDir = Join-Path $StageDir "server/bin/$($target.Target)"
    New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
    foreach ($binary in @('ghostlight', 'ghostlight-mcp-connector', 'ghostlight-browser-connector')) {
      $sourceName = "$binary-$($target.Target)$($target.Suffix)"
      $sourcePath = Join-Path $ArtifactsDir $sourceName
      if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
        throw "Required MCPB binary not found: $sourcePath"
      }
      Copy-Item -LiteralPath $sourcePath -Destination (Join-Path $targetDir "$binary$($target.Suffix)")
    }
  }

  New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
  Remove-Item -LiteralPath $PackagePath -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $ZipPath -Force -ErrorAction SilentlyContinue
  Compress-Archive -Path (Join-Path $StageDir '*') -DestinationPath $ZipPath -Force
  Move-Item -LiteralPath $ZipPath -Destination $PackagePath
}
finally {
  if (Test-Path -LiteralPath $StageDir) {
    $resolvedStage = (Resolve-Path -LiteralPath $StageDir).Path
    $stageParent = Split-Path -Parent $resolvedStage
    $stageLeaf = Split-Path -Leaf $resolvedStage
    if ($stageParent -ne $TempRoot -or $stageLeaf -notlike 'ghostlight-mcpb-stage-*') {
      throw "Refusing to remove unexpected MCPB stage path: $resolvedStage"
    }
    Remove-Item -LiteralPath $resolvedStage -Recurse -Force
  }
}

$PackagePath = (Resolve-Path $PackagePath).Path
$sizeMb = [math]::Round((Get-Item $PackagePath).Length / 1MB, 1)
Write-Host "Packaged: $PackagePath ($sizeMb MB)" -ForegroundColor Green
