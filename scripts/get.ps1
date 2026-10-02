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

if ([string]::IsNullOrWhiteSpace($env:USERPROFILE) -or -not [System.IO.Path]::IsPathRooted($env:USERPROFILE)) {
    throw "Ghostlight cannot locate this user's home directory."
}
$installDirectory = Join-Path $env:USERPROFILE ".ghostlight/bin"
$downloadDirectory = Join-Path ([System.IO.Path]::GetTempPath()) "ghostlight-download-$([Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $downloadDirectory | Out-Null
try {
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
        $destination = Join-Path $downloadDirectory "$component.exe"
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

    # Windows GUI-subsystem executables need an explicit waited process for CLI commands.
    $candidate = Join-Path $downloadDirectory "ghostlight.exe"
    $deploy = Start-Process -FilePath $candidate -ArgumentList @("deployment", "install", "`"$installDirectory`"") -WindowStyle Hidden -Wait -PassThru
    if ($deploy.ExitCode -ne 0) { throw "Ghostlight installation failed with exit code $($deploy.ExitCode)." }
}
finally {
    # The absolute temporary path was constructed directly under the OS temp directory.
    if ([System.IO.Path]::GetDirectoryName($downloadDirectory) -ne [System.IO.Path]::GetTempPath().TrimEnd('\', '/')) {
        throw "Unexpected Ghostlight download directory: $downloadDirectory"
    }
    Remove-Item -LiteralPath $downloadDirectory -Recurse -Force
}

$ghostlight = Join-Path $installDirectory "ghostlight.exe"
Write-Host "Ghostlight $version installed at $installDirectory"
if ($env:GHOSTLIGHT_NO_REGISTER -ne "1") {
    $setup = Start-Process -FilePath $ghostlight -ArgumentList "install" -WindowStyle Hidden -Wait -PassThru
    if ($setup.ExitCode -ne 0) {
        throw "Ghostlight installation did not complete. Run '$ghostlight doctor' for details."
    }
}
Write-Host "If anything does not connect, run '$ghostlight doctor'."
