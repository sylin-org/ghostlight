#Requires -Version 7
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Builds the workspace in an isolated target directory and hands the result to
# `ghostlight deployment install --development`. The installer owns locking,
# stopping running images, and copying. -Restore runs the installed binary's
# `deployment install --restore` instead and builds nothing.
[CmdletBinding()]
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
    # Stream each line as it arrives. A detached service started by the
    # installer can inherit the pipe ends and keep them open, so end of file
    # may never arrive: after the process exits, drain for a short bounded time
    # and then stop reading.
    $readers = @(
        @{ Reader = $process.StandardOutput; Sink = [Console]::Out; Pending = $null },
        @{ Reader = $process.StandardError; Sink = [Console]::Error; Pending = $null }
    )
    $deadline = $null
    while ($true) {
        foreach ($entry in $readers) {
            if ($null -eq $entry.Pending -and -not $entry.Done) { $entry.Pending = $entry.Reader.ReadLineAsync() }
        }
        $open = @($readers | Where-Object { -not $_.Done })
        if ($open.Count -eq 0) { break }
        if ($null -eq $deadline -and $process.HasExited) { $deadline = [DateTime]::UtcNow.AddMilliseconds(2000) }
        if ($null -ne $deadline -and [DateTime]::UtcNow -ge $deadline) { break }
        [void][System.Threading.Tasks.Task]::WaitAny([System.Threading.Tasks.Task[]]@($open | ForEach-Object { $_.Pending }), 100)
        foreach ($entry in $open) {
            if ($entry.Pending.IsCompleted) {
                $line = $entry.Pending.GetAwaiter().GetResult()
                $entry.Pending = $null
                if ($null -eq $line) { $entry.Done = $true } else { $entry.Sink.WriteLine($line) }
            }
        }
    }
    $process.WaitForExit()
    exit $process.ExitCode
}

if ($Restore) {
    $installed = Join-Path (Join-Path (Join-Path $HOME ".ghostlight") "bin") $exe
    $restore = @("deployment", "install", "--restore")
    if ($Plan) { $restore += "--plan" }
    Invoke-Ghostlight -Binary $installed -Arguments $restore
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
