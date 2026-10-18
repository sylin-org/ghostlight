# SPDX-License-Identifier: Apache-2.0 OR MIT
# irm https://sylin.org/ghostlight/get.ps1 | iex
#
# The Ghostlight 2.0 extension's setup page prints the one-line command that fetches this file;
# keep its URL working for 2.x.
#
# GHOSTLIGHT_VERSION installs exactly that release instead of the latest one, found by its tag
# (v<version>) through the GitHub releases API. A prerelease is installable this way and no other:
# the latest release never is. Reviewers and the owner use it before a cutover:
#   $env:GHOSTLIGHT_VERSION = "2.0.0-rc.1"; irm https://raw.githubusercontent.com/sylin-org/ghostlight/v2.0.0-rc.1/scripts/get.ps1 | iex
#
# -Restore installs this release over a development installation (a build the development loop
# installed), whatever version it is: the one allowed move down. It runs the downloaded
# service with `deployment install --restore` instead of `deployment install`. A piped script
# takes no switch, so run it as a script block:
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.ps1))) -Restore
#
# `irm | iex` runs this text in the person's own session. The body runs in a script block of its
# own, so its strict mode, preferences, functions and variables end with it.
& {
    param([switch]$Restore)

    $ErrorActionPreference = "Stop"
    # Windows PowerShell 5.1 slows a download badly while it draws a progress bar.
    $ProgressPreference = "SilentlyContinue"
    Set-StrictMode -Version Latest

    # Saves one release asset to Path and returns its lines. GitHub serves text assets such as
    # SHA256SUMS as application/octet-stream, so a response's Content is bytes, not text, in both
    # Windows PowerShell 5.1 and PowerShell 7: an asset is read as text only from the saved file.
    function Read-ReleaseText {
        param([string]$Uri, [string]$Path)
        Invoke-WebRequest -Uri $Uri -OutFile $Path -UseBasicParsing
        [System.IO.File]::ReadAllLines($Path)
    }

    # Returns the SHA-256 that the lines of a SHA256SUMS file bind to AssetName, or throws when
    # they do not bind it exactly once.
    function Get-BoundChecksum {
        param([string[]]$Lines, [string]$AssetName)
        $bound = @($Lines | Where-Object { $_ -match "^[0-9a-f]{64}  $([regex]::Escape($AssetName))$" })
        if ($bound.Count -ne 1) {
            throw "SHA256SUMS does not bind exactly one $AssetName."
        }
        $bound[0].Substring(0, 64)
    }

    $repository = "sylin-org/ghostlight"
    if (-not [Environment]::Is64BitOperatingSystem) {
        throw "Ghostlight publishes Windows binaries for x64 only."
    }

    $requested = $env:GHOSTLIGHT_VERSION
    if ([string]::IsNullOrWhiteSpace($requested)) {
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repository/releases/latest"
        if ($release.draft -or $release.prerelease -or $release.tag_name -notmatch '^v(?<version>[0-9]+\.[0-9]+\.[0-9]+)$') {
            throw "GitHub did not return a stable three-part Ghostlight release."
        }
        $version = $Matches.version
    } else {
        # A requested version is installed exactly, whatever the latest release is, prerelease included.
        if ($requested -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$') {
            throw "GHOSTLIGHT_VERSION must look like 2.0.0 or 2.0.0-rc.1."
        }
        $version = $requested
        try {
            $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repository/releases/tags/v$version"
        }
        catch {
            $response = $_.Exception.PSObject.Properties['Response']
            if ($response -and $response.Value -and [int]$response.Value.StatusCode -eq 404) {
                throw "Ghostlight release v$version does not exist."
            }
            throw
        }
        if ($release.draft -or $release.tag_name -ne "v$version") {
            throw "Ghostlight release v$version does not exist."
        }
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

    if ([string]::IsNullOrWhiteSpace($env:USERPROFILE) -or -not [System.IO.Path]::IsPathRooted($env:USERPROFILE)) {
        throw "Ghostlight cannot locate this user's home directory."
    }
    $installDirectory = Join-Path $env:USERPROFILE ".ghostlight/bin"
    $downloadDirectory = Join-Path ([System.IO.Path]::GetTempPath()) "ghostlight-download-$([Guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $downloadDirectory | Out-Null
    $provenanceForAll = $true
    try {
        $sumLines = Read-ReleaseText -Uri $assets["SHA256SUMS"] -Path (Join-Path $downloadDirectory "SHA256SUMS")
        foreach ($component in @("ghostlight", "ghostlight-service", "ghostlight-desktop")) {
            $assetName = "$component-x86_64-pc-windows-msvc.exe"
            if (-not $assets.ContainsKey($assetName)) {
                throw "Release $tag is missing $assetName."
            }
            $expected = Get-BoundChecksum -Lines $sumLines -AssetName $assetName
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
                    if ($LASTEXITCODE -ne 0) { $provenanceForAll = $false }
                } else {
                    $provenanceForAll = $false
                }
                Move-Item -LiteralPath $temporary -Destination $destination -Force
            }
            finally {
                if (Test-Path -LiteralPath $temporary) {
                    Remove-Item -LiteralPath $temporary -Force
                }
            }
        }

        if ($provenanceForAll) {
            Write-Host "Ghostlight $version downloaded. Checksums and build provenance verified."
        } else {
            Write-Host "Ghostlight $version downloaded. Checksums verified."
        }

        # The verified service installs the whole set (the edge, the service and the shell) into
        # ~/.ghostlight/bin from its own directory. Its stdout is discarded; its errors still show.
        $candidate = Join-Path $downloadDirectory "ghostlight-service.exe"
        $deployment = @("deployment", "install")
        if ($Restore) { $deployment += "--restore" }
        & $candidate @deployment | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "Ghostlight installation failed with exit code $LASTEXITCODE." }
    }
    finally {
        # The absolute temporary path was constructed directly under the OS temp directory.
        if ([System.IO.Path]::GetDirectoryName($downloadDirectory) -ne [System.IO.Path]::GetTempPath().TrimEnd('\', '/')) {
            throw "Unexpected Ghostlight download directory: $downloadDirectory"
        }
        Remove-Item -LiteralPath $downloadDirectory -Recurse -Force
    }

    $ghostlight = Join-Path $installDirectory "ghostlight.exe"
    if ($env:GHOSTLIGHT_NO_REGISTER -eq "1") {
        Write-Host "Ghostlight $version is installed in $installDirectory. Run 'ghostlight install' to connect your browsers and agents."
    } else {
        # The installed edge prints the install summary in this console.
        & $ghostlight install
        if ($LASTEXITCODE -ne 0) {
            throw "Ghostlight installation did not complete. Run '$ghostlight doctor' for details."
        }
    }
} @args
