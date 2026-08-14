# SPDX-License-Identifier: Apache-2.0 OR MIT
# irm https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.ps1 | iex

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repository = "sylin-org/ghostlight"
if (-not [Environment]::Is64BitOperatingSystem) {
    throw "Ghostlight publishes Windows binaries for x64 only."
}

$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repository/releases/latest"
if ($release.draft -or $release.prerelease -or $release.tag_name -notmatch '^v(?<version>[0-9]+\.[0-9]+\.[0-9]+)$') {
    throw "GitHub did not return a stable three-part Ghostlight release."
}
$version = $Matches.version
if (-not [string]::IsNullOrWhiteSpace($env:GHOSTLIGHT_VERSION) -and
    $env:GHOSTLIGHT_VERSION -ne $version) {
    throw "Latest Ghostlight is $version, not requested version $env:GHOSTLIGHT_VERSION."
}
$tag = $release.tag_name
$releaseRoot = "https://github.com/$repository/releases/download/$tag/"
$assets = @{}
foreach ($asset in $release.assets) {
    if ($asset.browser_download_url -notlike "$releaseRoot*") {
        throw "Release asset uses an unexpected download location: $($asset.browser_download_url)"
    }
    $assets[$asset.name] = $asset.browser_download_url
}
if (-not $assets.ContainsKey("SHA256SUMS")) {
    throw "Release $tag has no SHA256SUMS asset."
}
$sumLines = (Invoke-WebRequest -Uri $assets["SHA256SUMS"] -UseBasicParsing).Content -split "`n"

$installDirectory = Join-Path ([Environment]::GetFolderPath("UserProfile")) ".ghostlight/bin/v$version"
New-Item -ItemType Directory -Path $installDirectory -Force | Out-Null
foreach ($component in @("ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector")) {
    $assetName = "$component-x86_64-pc-windows-msvc.exe"
    if (-not $assets.ContainsKey($assetName)) {
        throw "Release $tag is missing $assetName."
    }
    $sumMatch = @($sumLines | Where-Object { $_ -match "^(?<hash>[0-9a-f]{64})  $([regex]::Escape($assetName))`r?$" })
    if ($sumMatch.Count -ne 1) {
        throw "SHA256SUMS does not bind exactly one $assetName."
    }
    [void]($sumMatch[0] -match '^(?<hash>[0-9a-f]{64})')
    $expected = $Matches.hash
    $destination = Join-Path $installDirectory "$component.exe"
    $temporary = "$destination.$PID.download"
    try {
        Invoke-WebRequest -Uri $assets[$assetName] -OutFile $temporary -UseBasicParsing
        $observed = (Get-FileHash -LiteralPath $temporary -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($observed -ne $expected) {
            throw "Checksum verification failed for $assetName."
        }
        $github = Get-Command gh -ErrorAction SilentlyContinue
        if ($github) {
            & gh attestation verify $temporary --repo $repository *> $null
            if ($LASTEXITCODE -eq 0) {
                Write-Host "  ${component}: checksum and build provenance verified"
            } else {
                Write-Host "  ${component}: checksum verified; GitHub provenance was not available"
            }
        } else {
            Write-Host "  ${component}: checksum verified"
        }
        Move-Item -LiteralPath $temporary -Destination $destination -Force
    }
    finally {
        if (Test-Path -LiteralPath $temporary) {
            Remove-Item -LiteralPath $temporary -Force
        }
    }
}

$ghostlight = Join-Path $installDirectory "ghostlight.exe"
Write-Host "Ghostlight $version installed at $installDirectory"
if ($env:GHOSTLIGHT_NO_REGISTER -ne "1") {
    & $ghostlight install
    if ($LASTEXITCODE -ne 0) {
        throw "Ghostlight installation did not complete. Run '$ghostlight doctor' for details."
    }
}
Write-Host "If anything does not connect, run '$ghostlight doctor'."
