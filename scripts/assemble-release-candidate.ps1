# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactRoot,
    [Parameter(Mandatory = $true)]
    [string[]]$SbomPath,
    [string]$OutputDirectory = "dist/release-candidate",
    [string]$Version,
    [string]$SourceRevision,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($Version)) {
    $cargo = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
    $match = [regex]::Match(
        $cargo,
        '(?ms)^\[workspace\.package\].*?^version\s*=\s*"(?<version>[^"]+)"'
    )
    if (-not $match.Success) {
        throw "Could not read workspace.package.version"
    }
    $Version = $match.Groups["version"].Value
}
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
    throw "Release version must be a three-part numeric version: $Version"
}

if ([string]::IsNullOrWhiteSpace($SourceRevision)) {
    $SourceRevision = (& git -C $repo rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "Could not resolve the source revision"
    }
}
$SourceRevision = $SourceRevision.ToLowerInvariant()
if ($SourceRevision -notmatch '^[0-9a-f]{40,64}$') {
    throw "SourceRevision is not a full Git object id: $SourceRevision"
}

$ArtifactRoot = [System.IO.Path]::GetFullPath($ArtifactRoot)
$SbomPath = @($SbomPath | ForEach-Object { [System.IO.Path]::GetFullPath($_) })
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
if (-not (Test-Path -LiteralPath $ArtifactRoot -PathType Container)) {
    throw "Artifact root does not exist: $ArtifactRoot"
}
if ($SbomPath.Count -ne 5) {
    throw "Candidate requires exactly five component SBOMs, found $($SbomPath.Count)"
}
foreach ($path in $SbomPath) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "SBOM does not exist: $path"
    }
}

if (Test-Path -LiteralPath $OutputDirectory) {
    if (-not $Force) {
        throw "Output directory already exists: $OutputDirectory (pass -Force to replace this exact directory)"
    }
    $resolvedOutput = [System.IO.Path]::GetFullPath($OutputDirectory)
    $outputName = [System.IO.Path]::GetFileName($resolvedOutput)
    if ($outputName -ne "release-candidate") {
        throw "Refusing to replace an output directory not named release-candidate: $resolvedOutput"
    }
    Remove-Item -LiteralPath $resolvedOutput -Recurse -Force
}

$assetDirectory = Join-Path $OutputDirectory "assets"
New-Item -ItemType Directory -Path $assetDirectory | Out-Null

function Resolve-OneArtifact {
    param(
        [string]$DirectoryName,
        [string]$Pattern
    )

    $directory = Join-Path $ArtifactRoot $DirectoryName
    if (-not (Test-Path -LiteralPath $directory -PathType Container)) {
        throw "Candidate input is missing artifact directory: $DirectoryName"
    }
    $matches = @(Get-ChildItem -LiteralPath $directory -File -Recurse -Filter $Pattern)
    if ($matches.Count -ne 1) {
        throw "Expected one $Pattern under $DirectoryName, found $($matches.Count)"
    }
    return $matches[0].FullName
}

$specifications = @(
    [ordered]@{
        kind = "client-bundle"
        target = "claude-desktop"
        directory = "derived"
        pattern = "ghostlight-v$Version.mcpb"
        name = "ghostlight-v$Version.mcpb"
    },
    [ordered]@{
        kind = "client-launcher"
        target = "npm"
        directory = "derived"
        pattern = "ghostlight-$Version.tgz"
        name = "ghostlight-$Version.tgz"
    },
    [ordered]@{
        kind = "native-package"
        target = "x86_64-pc-windows-msvc"
        directory = "native-x86_64-pc-windows-msvc"
        pattern = "*-setup.exe"
        name = "ghostlight-v$Version-x86_64-pc-windows-msvc-setup.exe"
    },
    [ordered]@{
        kind = "native-package"
        target = "x86_64-unknown-linux-gnu"
        directory = "native-x86_64-unknown-linux-gnu"
        pattern = "*.deb"
        name = "ghostlight-v$Version-x86_64-unknown-linux-gnu.deb"
    },
    [ordered]@{
        kind = "browser-adapter"
        target = "chromium-store"
        directory = "chrome-extension"
        pattern = "*.zip"
        name = "ghostlight-extension-v$Version.zip"
    }
)

$rawTargets = @(
    [ordered]@{ target = "x86_64-pc-windows-msvc"; extension = ".exe" },
    [ordered]@{ target = "x86_64-unknown-linux-gnu"; extension = "" }
)
foreach ($rawTarget in $rawTargets) {
    foreach ($component in @(
        "ghostlight",
        "ghostlight-mcp-connector",
        "ghostlight-browser-connector"
    )) {
        $name = "$component-$($rawTarget.target)$($rawTarget.extension)"
        $specifications += [ordered]@{
            kind = "raw-binary"
            target = "$component@$($rawTarget.target)"
            directory = "native-$($rawTarget.target)"
            pattern = $name
            name = $name
        }
    }
    $portableExtension = if ($rawTarget.target -eq "x86_64-pc-windows-msvc") { ".zip" } else { ".tar.gz" }
    $portableName = "ghostlight-v$Version-$($rawTarget.target)$portableExtension"
    $specifications += [ordered]@{
        kind = "portable-package"
        target = $rawTarget.target
        directory = "native-$($rawTarget.target)"
        pattern = $portableName
        name = $portableName
    }
}

$artifacts = [System.Collections.Generic.List[object]]::new()
foreach ($specification in $specifications) {
    $source = Resolve-OneArtifact `
        -DirectoryName $specification.directory `
        -Pattern $specification.pattern
    $destination = Join-Path $assetDirectory $specification.name
    Copy-Item -LiteralPath $source -Destination $destination
    $item = Get-Item -LiteralPath $destination
    $hash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant()
    [void]$artifacts.Add([pscustomobject][ordered]@{
        name = $item.Name
        kind = $specification.kind
        target = $specification.target
        bytes = $item.Length
        sha256 = $hash
    })
}

$expectedComponents = @(
    "ghostlight",
    "ghostlight-bridge",
    "ghostlight-browser-connector",
    "ghostlight-mcp-connector",
    "ghostlight-win-peer"
)
$observedComponents = [System.Collections.Generic.List[string]]::new()
foreach ($path in $SbomPath) {
    $document = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    $component = $document.metadata.component.name
    if ($document.bomFormat -ne "CycloneDX" -or
        $expectedComponents -notcontains $component -or
        $observedComponents.Contains($component)) {
        throw "Unexpected or duplicate component SBOM: $path"
    }
    [void]$observedComponents.Add($component)
    $sbomName = "ghostlight-v$Version-sbom-$component.cyclonedx.json"
    $sbomDestination = Join-Path $assetDirectory $sbomName
    Copy-Item -LiteralPath $path -Destination $sbomDestination
    $sbom = Get-Item -LiteralPath $sbomDestination
    [void]$artifacts.Add([pscustomobject][ordered]@{
        name = $sbom.Name
        kind = "sbom"
        target = $component
        bytes = $sbom.Length
        sha256 = (Get-FileHash -LiteralPath $sbomDestination -Algorithm SHA256).Hash.ToLowerInvariant()
    })
}
if ((@($observedComponents | Sort-Object) -join "`n") -ne
    (@($expectedComponents | Sort-Object) -join "`n")) {
    throw "Component SBOM set is incomplete"
}

$orderedArtifacts = @($artifacts | Sort-Object name)
$candidate = [ordered]@{
    schemaVersion = 1
    version = $Version
    sourceRevision = $SourceRevision
    generatedBy = "scripts/assemble-release-candidate.ps1"
    status = "release-candidate"
    artifacts = $orderedArtifacts
}
$candidateJson = $candidate | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText(
    (Join-Path $OutputDirectory "release-candidate.json"),
    $candidateJson + "`n",
    [System.Text.UTF8Encoding]::new($false)
)

$sumLines = @($orderedArtifacts | ForEach-Object { "$($_.sha256)  $($_.name)" })
[System.IO.File]::WriteAllText(
    (Join-Path $OutputDirectory "SHA256SUMS"),
    ($sumLines -join "`n") + "`n",
    [System.Text.UTF8Encoding]::new($false)
)

& (Join-Path $PSScriptRoot "check-release-candidate.ps1") -CandidateDirectory $OutputDirectory

Write-Output "Release candidate assembled at $OutputDirectory"
