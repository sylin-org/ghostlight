# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("windows", "linux", "macos")]
    [string]$Platform,
    [Parameter(Mandatory = $true)]
    [string]$Artifact
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Artifact = [System.IO.Path]::GetFullPath($Artifact)
if (-not (Test-Path -LiteralPath $Artifact)) {
    throw "Native package artifact does not exist: $Artifact"
}

$binaries = @(
    "ghostlight",
    "ghostlight-mcp-connector",
    "ghostlight-browser-connector"
)

switch ($Platform) {
    "windows" {
        $listing = (& 7z l $Artifact) -join "`n"
        if ($LASTEXITCODE -ne 0) {
            throw "7z could not inspect the NSIS artifact"
        }
        foreach ($binary in $binaries) {
            if ($listing -notmatch "(?m)\s$([regex]::Escape($binary))\.exe$") {
                throw "NSIS package is missing $binary.exe"
            }
        }
        if ($listing -match 'ghostlight-(mcp|browser)-connector-[a-z0-9_.-]+\.exe') {
            throw "NSIS package exposed a Tauri target-triple staging name"
        }
    }
    "linux" {
        $listing = (& dpkg-deb --contents $Artifact) -join "`n"
        if ($LASTEXITCODE -ne 0) {
            throw "dpkg-deb could not inspect the Debian artifact"
        }
        foreach ($binary in $binaries) {
            if ($listing -notmatch "(?m)\./usr/bin/$([regex]::Escape($binary))$") {
                throw "Debian package is missing /usr/bin/$binary"
            }
        }
        $manifestDestinations = @(
            "etc/opt/chrome/native-messaging-hosts",
            "etc/opt/edge/native-messaging-hosts",
            "etc/brave/native-messaging-hosts",
            "etc/chromium/native-messaging-hosts"
        )
        foreach ($destination in $manifestDestinations) {
            if ($listing -notmatch "(?m)\./$([regex]::Escape($destination))/org\.sylin\.ghostlight\.json$") {
                throw "Debian package is missing the $destination native-host manifest"
            }
        }

        $tempBase = [System.IO.Path]::GetTempPath()
        $tempRoot = Join-Path $tempBase ("ghostlight-deb-check-" + [guid]::NewGuid().ToString("N"))
        New-Item -ItemType Directory -Path $tempRoot | Out-Null
        try {
            & dpkg-deb --extract $Artifact $tempRoot
            if ($LASTEXITCODE -ne 0) {
                throw "dpkg-deb could not extract the Debian artifact"
            }
            foreach ($destination in $manifestDestinations) {
                $path = Join-Path $tempRoot "$destination/org.sylin.ghostlight.json"
                $manifest = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
                if ($manifest.name -ne "org.sylin.ghostlight" -or
                    $manifest.path -ne "/usr/bin/ghostlight-browser-connector" -or
                    $manifest.type -ne "stdio") {
                    throw "Debian native-host manifest is not bound to the packaged connector: $path"
                }
                if (@($manifest.allowed_origins).Count -ne 2) {
                    throw "Debian native-host manifest does not carry both fixed extension identities"
                }
            }
        }
        finally {
            $resolved = [System.IO.Path]::GetFullPath($tempRoot)
            if (-not $resolved.StartsWith([System.IO.Path]::GetFullPath($tempBase), [System.StringComparison]::OrdinalIgnoreCase) -or
                -not ([System.IO.Path]::GetFileName($resolved)).StartsWith("ghostlight-deb-check-")) {
                throw "Refusing to clean unexpected package-check path: $resolved"
            }
            if (Test-Path -LiteralPath $resolved) {
                Remove-Item -LiteralPath $resolved -Recurse -Force
            }
        }
    }
    "macos" {
        $macosDirectory = Join-Path $Artifact "Contents/MacOS"
        foreach ($binary in $binaries) {
            if (-not (Test-Path -LiteralPath (Join-Path $macosDirectory $binary) -PathType Leaf)) {
                throw "macOS application bundle is missing Contents/MacOS/$binary"
            }
        }
    }
}

Write-Output "$Platform native package contains the complete Ghostlight sibling set"
