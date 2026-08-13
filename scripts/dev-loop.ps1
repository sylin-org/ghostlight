#Requires -Version 7
# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [ValidateSet("Plan", "Deploy")]
    [string]$Action = "Plan",
    [ValidateSet("orchestrator", "mcp-connector", "browser-connector")]
    [string[]]$Component = @("orchestrator"),
    [ValidateSet("debug", "release")]
    [string]$Profile = "release",
    [string]$TargetDirectory = ".target-dev-loop",
    [string]$LiveDirectory = "target/release",
    [switch]$RegisterNativeHost,
    [switch]$NoStart
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
function Resolve-RepositoryPath {
    param([string]$Path)

    if ([System.IO.Path]::IsPathFullyQualified($Path)) {
        return [System.IO.Path]::GetFullPath($Path)
    }
    return [System.IO.Path]::GetFullPath((Join-Path $repo $Path))
}

$TargetDirectory = Resolve-RepositoryPath -Path $TargetDirectory
$LiveDirectory = Resolve-RepositoryPath -Path $LiveDirectory
$comparison = if ($IsWindows) {
    [System.StringComparison]::OrdinalIgnoreCase
}
else {
    [System.StringComparison]::Ordinal
}

function Assert-RepositoryPath {
    param([string]$Path, [string]$Name)

    $root = $repo.TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    ) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $Path.StartsWith($root, $comparison)) {
        throw "$Name must remain inside the repository: $Path"
    }
}

function Get-ExactImageProcesses {
    param([string[]]$ImagePaths)

    $resolved = @($ImagePaths | ForEach-Object { [System.IO.Path]::GetFullPath($_) })
    return @(Get-Process | ForEach-Object {
        $process = $_
        try {
            if ([string]::IsNullOrWhiteSpace($process.Path)) {
                return
            }
            $processPath = [System.IO.Path]::GetFullPath($process.Path)
            if (@($resolved | Where-Object { $processPath.Equals($_, $comparison) }).Count -gt 0) {
                $process
            }
        }
        catch {
            # Some system processes deny image-path access. They cannot match a repository path.
        }
    })
}

function Copy-WithRetry {
    param([string]$Source, [string]$Destination)

    $attempt = 0
    while ($true) {
        try {
            Copy-Item -LiteralPath $Source -Destination $Destination -Force
            return
        }
        catch {
            $attempt += 1
            if ($attempt -ge 30) {
                throw
            }
            Start-Sleep -Milliseconds 200
        }
    }
}

Assert-RepositoryPath -Path $TargetDirectory -Name "TargetDirectory"
Assert-RepositoryPath -Path $LiveDirectory -Name "LiveDirectory"

$extension = if ($IsWindows) { ".exe" } else { "" }
$definitions = [ordered]@{
    "orchestrator" = [ordered]@{
        package = "ghostlight"
        binary = "ghostlight$extension"
    }
    "mcp-connector" = [ordered]@{
        package = "ghostlight-mcp-connector"
        binary = "ghostlight-mcp-connector$extension"
    }
    "browser-connector" = [ordered]@{
        package = "ghostlight-browser-connector"
        binary = "ghostlight-browser-connector$extension"
    }
}
$selected = @($Component | Select-Object -Unique)
$sourceRoot = Join-Path $TargetDirectory $Profile
$transfers = @($selected | ForEach-Object {
    $definition = $definitions[$_]
    [pscustomobject]@{
        component = $_
        package = $definition.package
        source = Join-Path $sourceRoot $definition.binary
        destination = Join-Path $LiveDirectory $definition.binary
    }
})

Write-Output "Ghostlight development loop: $Action"
Write-Output "Profile: $Profile"
Write-Output "Build root: $TargetDirectory"
Write-Output "Live root: $LiveDirectory"
foreach ($transfer in $transfers) {
    Write-Output "Component: $($transfer.component)"
    Write-Output "  source: $($transfer.source)"
    Write-Output "  destination: $($transfer.destination)"
}

$running = @(Get-ExactImageProcesses -ImagePaths @($transfers.destination))
if ($running.Count -eq 0) {
    Write-Output "Matching live processes: none"
}
else {
    foreach ($process in $running) {
        Write-Output "Matching live process: pid=$($process.Id) path=$($process.Path)"
    }
}

if ($Action -eq "Plan") {
    Write-Output "No build, process stop, copy, registration, or launch was performed."
    return
}

$cargoArguments = @("build", "--locked", "--target-dir", $TargetDirectory)
foreach ($name in $selected) {
    $cargoArguments += @("-p", $definitions[$name].package)
}
if ($Profile -eq "release") {
    $cargoArguments += "--release"
}

Push-Location $repo
try {
    & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}
foreach ($transfer in $transfers) {
    if (-not (Test-Path -LiteralPath $transfer.source -PathType Leaf)) {
        throw "Build did not produce $($transfer.source)"
    }
}

[System.IO.Directory]::CreateDirectory($LiveDirectory) | Out-Null
$deploymentLock = Join-Path $LiveDirectory "deploy.lock"
[System.IO.File]::WriteAllText($deploymentLock, "dev-loop`n")
try {
    $running = @(Get-ExactImageProcesses -ImagePaths @($transfers.destination))
    foreach ($process in $running) {
        Write-Output "Stopping exact live process: pid=$($process.Id) path=$($process.Path)"
        Stop-Process -Id $process.Id
    }
    foreach ($process in $running) {
        Wait-Process -Id $process.Id -Timeout 10 -ErrorAction SilentlyContinue
    }

    foreach ($transfer in $transfers) {
        Copy-WithRetry -Source $transfer.source -Destination $transfer.destination
        Write-Output "Replaced: $($transfer.destination)"
    }

    if ($RegisterNativeHost) {
        $orchestrator = Join-Path $LiveDirectory "ghostlight$extension"
        if (-not (Test-Path -LiteralPath $orchestrator -PathType Leaf)) {
            throw "Native-host registration requires the live orchestrator: $orchestrator"
        }
        & $orchestrator native-host install
        if ($LASTEXITCODE -ne 0) {
            throw "Native-host registration failed with exit code $LASTEXITCODE"
        }
    }
}
finally {
    if (Test-Path -LiteralPath $deploymentLock -PathType Leaf) {
        Remove-Item -LiteralPath $deploymentLock -Force
    }
}

if (-not $NoStart -and $selected -contains "orchestrator") {
    $orchestrator = Join-Path $LiveDirectory "ghostlight$extension"
    Start-Process -FilePath $orchestrator -WorkingDirectory $LiveDirectory
    Write-Output "Started: $orchestrator"
}
if ($selected -contains "browser-connector") {
    Write-Output "Reload the unpacked extension at chrome://extensions before browser validation."
}
Write-Output "Development deployment completed."
