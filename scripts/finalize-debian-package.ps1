# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$Artifact
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Artifact = [System.IO.Path]::GetFullPath($Artifact)
if (-not (Test-Path -LiteralPath $Artifact)) {
    throw "Debian package artifact does not exist: $Artifact"
}

$prefix = "ghostlight-deb-finalize-"
$root = Join-Path ([System.IO.Path]::GetTempPath()) ($prefix + [guid]::NewGuid().ToString("N"))
$output = Join-Path ([System.IO.Path]::GetDirectoryName($Artifact)) (
    "." + [System.IO.Path]::GetFileName($Artifact) + "." + [guid]::NewGuid().ToString("N") + ".tmp"
)
$conffiles = @(
    "/etc/opt/chrome/native-messaging-hosts/org.sylin.ghostlight.json",
    "/etc/opt/edge/native-messaging-hosts/org.sylin.ghostlight.json",
    "/etc/brave/native-messaging-hosts/org.sylin.ghostlight.json",
    "/etc/chromium/native-messaging-hosts/org.sylin.ghostlight.json"
)

try {
    New-Item -ItemType Directory -Path $root | Out-Null
    & dpkg-deb --raw-extract $Artifact $root
    if ($LASTEXITCODE -ne 0) {
        throw "dpkg-deb could not extract the Debian package"
    }

    $controlDirectory = Join-Path $root "DEBIAN"
    if (-not (Test-Path -LiteralPath (Join-Path $controlDirectory "control"))) {
        throw "Extracted Debian package has no control metadata"
    }

    # Tauri 2.9 uses the display name for this path instead of the Debian
    # package name. Normalize it while this script already owns finalization.
    $displayDocDirectory = Join-Path $root "usr/share/doc/Ghostlight"
    $displayChangelog = Join-Path $displayDocDirectory "changelog.gz"
    $packageDocDirectory = Join-Path $root "usr/share/doc/ghostlight"
    $packageChangelog = Join-Path $packageDocDirectory "changelog.gz"
    if (-not (Test-Path -LiteralPath $displayChangelog -PathType Leaf)) {
        throw "Tauri package is missing its generated changelog: $displayChangelog"
    }
    if (-not (Test-Path -LiteralPath $packageDocDirectory -PathType Container)) {
        New-Item -ItemType Directory -Path $packageDocDirectory | Out-Null
    }
    Move-Item -LiteralPath $displayChangelog -Destination $packageChangelog
    if (@(Get-ChildItem -LiteralPath $displayDocDirectory).Count -eq 0) {
        Remove-Item -LiteralPath $displayDocDirectory
    }

    # Tauri does not carry man pages, and lintian reports their absence for every one of the three
    # executables. They are installed here, before md5sums are recomputed below, so the checksums
    # cover them like any other payload file.
    $manSource = Join-Path $PSScriptRoot "../packaging/linux/man"
    $manDirectory = Join-Path $root "usr/share/man/man1"
    if (-not (Test-Path -LiteralPath $manDirectory -PathType Container)) {
        New-Item -ItemType Directory -Path $manDirectory -Force | Out-Null
    }
    foreach ($page in @("ghostlight.1", "ghostlight-mcp-connector.1", "ghostlight-browser-connector.1")) {
        $from = Join-Path $manSource $page
        if (-not (Test-Path -LiteralPath $from -PathType Leaf)) {
            throw "Required man page is missing: $from"
        }
        # Debian policy expects compressed pages. gzip -n keeps the output reproducible by leaving
        # the source timestamp and name out of the header.
        $to = Join-Path $manDirectory ($page + ".gz")
        & gzip -9 -n -c $from > $to
        if ($LASTEXITCODE -ne 0) {
            throw "Could not compress man page: $from"
        }
    }

    [System.IO.File]::WriteAllLines(
        (Join-Path $controlDirectory "conffiles"),
        $conffiles,
        [System.Text.UTF8Encoding]::new($false)
    )
    $dataFiles = @(
        Get-ChildItem -LiteralPath $root -File -Recurse |
            Where-Object { -not $_.FullName.StartsWith($controlDirectory + [System.IO.Path]::DirectorySeparatorChar) } |
            ForEach-Object {
                $relative = [System.IO.Path]::GetRelativePath($root, $_.FullName).Replace('\', '/')
                [pscustomobject]@{
                    relative = $relative
                    line = (Get-FileHash -LiteralPath $_.FullName -Algorithm MD5).Hash.ToLowerInvariant() + "  " + $relative
                }
            } |
            Sort-Object relative
    )
    [System.IO.File]::WriteAllLines(
        (Join-Path $controlDirectory "md5sums"),
        @($dataFiles | ForEach-Object { $_.line }),
        [System.Text.UTF8Encoding]::new($false)
    )

    & dpkg-deb -Zxz --root-owner-group --build $root $output
    if ($LASTEXITCODE -ne 0) {
        throw "dpkg-deb could not rebuild the finalized Debian package"
    }
    Move-Item -LiteralPath $output -Destination $Artifact -Force
}
finally {
    if (Test-Path -LiteralPath $output) {
        Remove-Item -LiteralPath $output -Force
    }
    $tempBase = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    $resolvedRoot = [System.IO.Path]::GetFullPath($root)
    if ($resolvedRoot.StartsWith($tempBase, [System.StringComparison]::OrdinalIgnoreCase) -and
        ([System.IO.Path]::GetFileName($resolvedRoot)).StartsWith($prefix) -and
        (Test-Path -LiteralPath $resolvedRoot)) {
        Remove-Item -LiteralPath $resolvedRoot -Recurse -Force
    }
}

Write-Output "Debian native-host manifests are marked as configuration files"
