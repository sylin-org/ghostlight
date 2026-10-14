#Requires -Version 7
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Builds the workspace in an isolated target directory and hands the result to
# `ghostlight deployment install --development`. The installer owns locking,
# stopping running images, and copying. -Restore runs the installed binary's
# `deployment install --restore` instead and builds nothing.
param(
    [switch]$Restore,
    [switch]$Plan,
    [ValidateSet("release", "debug")]
    [string]$Profile = "release",
    [string]$TargetDirectory = ".target-dev-loop"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$exe = if ($IsWindows) { "ghostlight.exe" } else { "ghostlight" }

# The binary is GUI-subsystem on Windows, so its output is captured through a
# redirected process rather than the console.
function Invoke-Ghostlight {
    param([string]$Binary, [string[]]$Arguments)

    $info = [System.Diagnostics.ProcessStartInfo]::new($Binary)
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $process = [System.Diagnostics.Process]::Start($info)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    [Console]::Out.Write($stdout.GetAwaiter().GetResult())
    [Console]::Error.Write($stderr.GetAwaiter().GetResult())
    exit $process.ExitCode
}

if ($Restore) {
    $installed = Join-Path (Join-Path (Join-Path $HOME ".ghostlight") "bin") $exe
    Invoke-Ghostlight -Binary $installed -Arguments @("deployment", "install", "--restore")
}

$target = if ([System.IO.Path]::IsPathFullyQualified($TargetDirectory)) {
    [System.IO.Path]::GetFullPath($TargetDirectory)
}
else {
    [System.IO.Path]::GetFullPath((Join-Path $repo $TargetDirectory))
}
$comparison = if ($IsWindows) { [System.StringComparison]::OrdinalIgnoreCase } else { [System.StringComparison]::Ordinal }
$root = $repo.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $target.StartsWith($root, $comparison)) {
    throw "TargetDirectory must remain inside the repository: $target"
}

$env:CARGO_TARGET_DIR = $target
$build = @("build", "--workspace", "--locked")
if ($Profile -eq "release") { $build += "--release" }
Push-Location $repo
try {
    & cargo @build
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
finally { Pop-Location }

$install = @("deployment", "install", "--development")
if ($Plan) { $install += "--plan" }
Invoke-Ghostlight -Binary (Join-Path (Join-Path $target $Profile) $exe) -Arguments $install
