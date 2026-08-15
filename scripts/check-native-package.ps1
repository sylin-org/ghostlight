# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("windows", "linux")]
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
$repo = Split-Path -Parent $PSScriptRoot
$legalFiles = @(
    [pscustomobject]@{
        name = "Apache-2.0.txt"
        source = Join-Path $repo "LICENSE"
    },
    [pscustomobject]@{
        name = "MIT.txt"
        source = Join-Path $repo "docs/licenses/MIT.txt"
    },
    [pscustomobject]@{
        name = "LicenseRef-Ghostlight-Commercial.txt"
        source = Join-Path $repo "docs/licenses/LicenseRef-Ghostlight-Commercial.txt"
    },
    [pscustomobject]@{
        name = "LICENSING.md"
        source = Join-Path $repo "LICENSING.md"
    }
)

function Assert-LegalPayload {
    param([string]$Root)

    foreach ($legal in $legalFiles) {
        $matches = @(Get-ChildItem -LiteralPath $Root -File -Recurse -Filter $legal.name)
        if ($matches.Count -ne 1) {
            throw "Native package must contain one $($legal.name), found $($matches.Count)"
        }
        $sourceHash = (Get-FileHash -LiteralPath $legal.source -Algorithm SHA256).Hash
        $packageHash = (Get-FileHash -LiteralPath $matches[0].FullName -Algorithm SHA256).Hash
        if ($sourceHash -ne $packageHash) {
            throw "Native package contains the wrong bytes for $($legal.name)"
        }
    }
}

function New-CheckedTemporaryDirectory {
    param([string]$Prefix)

    $base = [System.IO.Path]::GetTempPath()
    $path = Join-Path $base ($Prefix + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $path | Out-Null
    return $path
}

function Remove-CheckedTemporaryDirectory {
    param([string]$Path, [string]$Prefix)

    $base = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    $resolved = [System.IO.Path]::GetFullPath($Path)
    if (-not $resolved.StartsWith($base, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not ([System.IO.Path]::GetFileName($resolved)).StartsWith($Prefix)) {
        throw "Refusing to clean unexpected package-check path: $resolved"
    }
    if (Test-Path -LiteralPath $resolved) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

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
        $tempRoot = New-CheckedTemporaryDirectory -Prefix "ghostlight-nsis-check-"
        try {
            & 7z x $Artifact "-o$tempRoot" -y *> $null
            if ($LASTEXITCODE -ne 0) {
                throw "7z could not extract the NSIS artifact"
            }
            Assert-LegalPayload -Root $tempRoot
        }
        finally {
            Remove-CheckedTemporaryDirectory `
                -Path $tempRoot `
                -Prefix "ghostlight-nsis-check-"
        }
    }
    "linux" {
        $listing = (& dpkg-deb --contents $Artifact) -join "`n"
        if ($LASTEXITCODE -ne 0) {
            throw "dpkg-deb could not inspect the Debian artifact"
        }
        foreach ($binary in $binaries) {
            if ($listing -notmatch "(?m)(?:\./)?usr/bin/$([regex]::Escape($binary))$") {
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
            if ($listing -notmatch "(?m)(?:\./)?$([regex]::Escape($destination))/org\.sylin\.ghostlight\.json$") {
                throw "Debian package is missing the $destination native-host manifest"
            }
        }

        $tempRoot = New-CheckedTemporaryDirectory -Prefix "ghostlight-deb-check-"
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
            $desktopEntries = @(
                Get-ChildItem `
                    -LiteralPath (Join-Path $tempRoot "usr/share/applications") `
                    -File `
                    -Filter "*.desktop"
            )
            if ($desktopEntries.Count -ne 1) {
                throw "Debian package must contain one desktop entry, found $($desktopEntries.Count)"
            }
            $desktopEntry = Get-Content -LiteralPath $desktopEntries[0].FullName -Raw
            if ($desktopEntry -notmatch '(?m)^Exec=.*ghostlight.* open$' -or
                $desktopEntry -notmatch '(?m)^X-Ghostlight-Owned=true$') {
                throw "Debian desktop entry must use the explicit Ghostlight open intent"
            }
            Assert-LegalPayload -Root $tempRoot
        }
        finally {
            Remove-CheckedTemporaryDirectory `
                -Path $tempRoot `
                -Prefix "ghostlight-deb-check-"
        }
    }
}

Write-Output "$Platform native package contains the complete Ghostlight sibling set"
