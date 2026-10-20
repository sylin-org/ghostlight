# SPDX-License-Identifier: Apache-2.0 OR MIT
# irm https://sylin.org/ghostlight/get.ps1 | iex
#
# The Ghostlight 2.0 extension's setup page prints the one-line command that fetches this file;
# keep its URL working for 2.x.
#
# The install needs no GitHub account and no GitHub tool. Every image is checked against the
# release's SHA256SUMS before anything runs. docs/trust/supply-chain.md shows how to check build
# provenance by hand.
#
# GHOSTLIGHT_VERSION installs exactly that release instead of the latest one, found by its tag
# (v<version>) through the GitHub releases API. A prerelease is installable this way and no other:
# the latest release never is:
#   $env:GHOSTLIGHT_VERSION = "2.0.0"; irm https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.ps1 | iex
#
# GHOSTLIGHT_FROM installs from a release bundle on this machine instead of GitHub, for testing a
# build before it is released. It names a folder: the release workflow's `release-candidate`
# artifact, unpacked (SHA256SUMS beside an assets folder), or a folder of release downloads
# (SHA256SUMS beside the images). Every image is checked against SHA256SUMS exactly as a download
# is, and anything it does not bind is refused. A bundle is only as trustworthy as its source. It
# cannot be combined with GHOSTLIGHT_VERSION:
#   $env:GHOSTLIGHT_FROM = "$HOME\Downloads\release-candidate"; irm https://raw.githubusercontent.com/sylin-org/ghostlight/<revision>/scripts/get.ps1 | iex
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
    $from = $env:GHOSTLIGHT_FROM
    $local = -not [string]::IsNullOrWhiteSpace($from)
    if ($local) {
        if (-not [string]::IsNullOrWhiteSpace($requested)) {
            throw "GHOSTLIGHT_FROM and GHOSTLIGHT_VERSION cannot be combined."
        }
        if (-not (Test-Path -LiteralPath $from -PathType Container)) {
            throw "GHOSTLIGHT_FROM is not a folder: $from"
        }
        $from = (Resolve-Path -LiteralPath $from).ProviderPath
        # The release workflow's bundle keeps its images in assets; a folder of release downloads
        # keeps them beside SHA256SUMS.
        $images = Join-Path $from "assets"
        if (-not (Test-Path -LiteralPath $images -PathType Container)) { $images = $from }
        $localSums = Join-Path $from "SHA256SUMS"
        if (-not (Test-Path -LiteralPath $localSums -PathType Leaf)) {
            throw "$from has no SHA256SUMS."
        }
        # The bundle's manifest names its version; a folder of downloads names none.
        $version = $null
        $manifest = Join-Path $from "release-candidate.json"
        if (Test-Path -LiteralPath $manifest -PathType Leaf) {
            $named = (Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json).PSObject.Properties['version']
            if ($named -and "$($named.Value)" -match '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$') { $version = "$($named.Value)" }
        }
    } elseif ([string]::IsNullOrWhiteSpace($requested)) {
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
    if (-not $local) {
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
    }

    $downloadDirectory = Join-Path ([System.IO.Path]::GetTempPath()) "ghostlight-download-$([Guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $downloadDirectory | Out-Null
    try {
        if ($local) {
            $sumsCopy = Join-Path $downloadDirectory "SHA256SUMS"
            Copy-Item -LiteralPath $localSums -Destination $sumsCopy
            $sumLines = [System.IO.File]::ReadAllLines($sumsCopy)
        } else {
            $sumLines = Read-ReleaseText -Uri $assets["SHA256SUMS"] -Path (Join-Path $downloadDirectory "SHA256SUMS")
        }
        foreach ($component in @("ghostlight", "ghostlight-service", "ghostlight-desktop")) {
            $assetName = "$component-x86_64-pc-windows-msvc.exe"
            if ($local) {
                $source = Join-Path $images $assetName
                if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
                    throw "$images has no $assetName."
                }
            } elseif (-not $assets.ContainsKey($assetName)) {
                throw "Release $tag is missing $assetName."
            }
            $expected = Get-BoundChecksum -Lines $sumLines -AssetName $assetName
            $destination = Join-Path $downloadDirectory "$component.exe"
            $temporary = "$destination.$PID.download"
            try {
                if ($local) {
                    # The copy is what runs, so the copy is what is checked.
                    Copy-Item -LiteralPath $source -Destination $temporary
                } else {
                    Invoke-WebRequest -Uri $assets[$assetName] -OutFile $temporary -UseBasicParsing
                }
                $observed = (Get-FileHash -LiteralPath $temporary -Algorithm SHA256).Hash.ToLowerInvariant()
                if ($observed -ne $expected) {
                    throw "Checksum verification failed for $assetName."
                }
                Move-Item -LiteralPath $temporary -Destination $destination -Force
            }
            finally {
                if (Test-Path -LiteralPath $temporary) {
                    Remove-Item -LiteralPath $temporary -Force
                }
            }
        }

        $product = if ($version) { "Ghostlight $version" } else { "Ghostlight" }
        $obtained = if ($local) { "copied from $from" } else { "downloaded" }
        Write-Host "$product $obtained. Checksums verified."

        # The verified service installs the whole set (the edge, the service and the shell) into
        # the Ghostlight home from its own directory. Its stdout is discarded; its errors still show.
        $service = Join-Path $downloadDirectory "ghostlight-service.exe"
        $deployment = @("deployment", "install")
        if ($Restore) { $deployment += "--restore" }
        & $service @deployment | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "Ghostlight installation failed with exit code $LASTEXITCODE." }

        if ($env:GHOSTLIGHT_NO_REGISTER -eq "1") {
            Write-Host "$product is installed. Run 'ghostlight install' to connect your browsers and agents."
        } else {
            # The same verified service hands `install` to the copy it just installed, which prints
            # the install summary in this console.
            & $service install
            if ($LASTEXITCODE -ne 0) {
                # The ghostlight command may not exist yet, so the same downloaded service, which
                # reads the same home, reports what it finds before its temporary directory goes.
                & $service doctor
                throw "Ghostlight installation did not complete."
            }
        }
    }
    finally {
        # The absolute temporary path was constructed directly under the OS temp directory.
        if ([System.IO.Path]::GetDirectoryName($downloadDirectory) -ne [System.IO.Path]::GetTempPath().TrimEnd('\', '/')) {
            throw "Unexpected Ghostlight download directory: $downloadDirectory"
        }
        Remove-Item -LiteralPath $downloadDirectory -Recurse -Force
    }
} @args
