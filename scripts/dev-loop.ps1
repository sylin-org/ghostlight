#Requires -Version 7
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Builds the workspace in an isolated target directory and hands the result to
# `ghostlight-service deployment install --development`. The installer owns locking,
# stopping running images, and copying: it installs the build into ~/.ghostlight/bin as the
# development channel, replacing the one Ghostlight on the machine in place. Going back to a
# release is a release carrier's `deployment install --restore` (npm or the get scripts), not
# this script.
[CmdletBinding()]
param(
    [switch]$Plan,
    [ValidateSet("release", "debug")]
    [string]$Profile = "release",
    [string]$TargetDirectory = ".target-dev-loop"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$exe = if ($IsWindows) { "ghostlight-service.exe" } else { "ghostlight-service" }

# The installer's output and exit code are captured through a redirected process, so a failure
# reaches this script as an exit code on every host.
function Invoke-Ghostlight {
    param([string]$Binary, [string[]]$Arguments)

    $info = [System.Diagnostics.ProcessStartInfo]::new($Binary)
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $process = [System.Diagnostics.Process]::Start($info)
    # Standard error is collected on a task so neither pipe can fill and block the process;
    # standard output streams line by line. Both reach end of file when the process exits,
    # because a service the installer starts never holds these pipes.
    $errors = $process.StandardError.ReadToEndAsync()
    while ($null -ne ($line = $process.StandardOutput.ReadLine())) { [Console]::Out.WriteLine($line) }
    [Console]::Error.Write($errors.GetAwaiter().GetResult())
    $process.WaitForExit()
    exit $process.ExitCode
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

$build = @("build", "--workspace", "--locked", "--target-dir", $target)
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
