#Requires -Version 7.0
<#
.SYNOPSIS
    A complete governed browser journey driven entirely by `ghostlight call`.

.DESCRIPTION
    No MCP client and no model: every step below is one command-line invocation, governed and
    audited exactly as an agent's call would be.

    Each step is its own process, and they all reach the same tabs because a Ghostlight session is
    its caller -- this shell -- rather than a connection (ADR-0106). That is why the handle from the
    first step is still good in the last one.

.PARAMETER Url
    The page to open. Defaults to example.com.

.PARAMETER Ghostlight
    Path to the ghostlight executable. Defaults to PATH, then the repository build.

.PARAMETER OutputPath
    Where the screenshot is written.

.EXAMPLE
    ./scripts/browser-journey.ps1
    ./scripts/browser-journey.ps1 -Url https://example.org -OutputPath capture.jpg
#>
[CmdletBinding()]
param(
    [string] $Url = 'https://example.com',
    [string] $Ghostlight,
    [string] $OutputPath = (Join-Path ([System.IO.Path]::GetTempPath()) 'ghostlight-journey.jpg')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Resolve-Ghostlight {
    if ($Ghostlight) { return (Resolve-Path -LiteralPath $Ghostlight).Path }
    $found = Get-Command 'ghostlight' -CommandType Application -ErrorAction SilentlyContinue
    if ($found) { return $found.Source }

    $suffix = if ($IsWindows) { '.exe' } else { '' }
    $repository = Split-Path -Parent $PSScriptRoot
    foreach ($build in @('.target-ghostlight-1.0/debug', 'target/release', 'target/debug')) {
        $candidate = Join-Path $repository "$build/ghostlight$suffix"
        if (Test-Path -LiteralPath $candidate) { return (Resolve-Path -LiteralPath $candidate).Path }
    }
    throw 'Could not find ghostlight. Put it on PATH or pass -Ghostlight.'
}

$exe = Resolve-Ghostlight

# One call, reported as a row. The exit code comes from Ghostlight rather than being invented here,
# so a governed refusal (2) stays distinguishable from a failure (4) and an uncertain effect (6).
# Stop at the first unsuccessful call. A later cleanup refusal must not replace uncertain work,
# and neither dependent work nor retries are safe after an outcome the script cannot establish.
function Step {
    param([string] $Name, [string] $Tool, [hashtable] $Body = @{}, [string[]] $Extra = @())

    # Ghostlight is a Windows GUI-subsystem executable. Explicit redirection and waiting capture
    # its output and exit reliably; the ordinary & pipeline and $LASTEXITCODE do not.
    $start = [System.Diagnostics.ProcessStartInfo]::new($exe)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in @('call', $Tool, ($Body | ConvertTo-Json -Compress), '--json') + $Extra) {
        $start.ArgumentList.Add($argument)
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $start
    try {
        $null = $process.Start()
        # Drain both streams concurrently so a long diagnostic cannot block the child.
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        $text = $stdout.GetAwaiter().GetResult()
        $diagnostic = $stderr.GetAwaiter().GetResult()
        $code = $process.ExitCode
    } finally {
        $process.Dispose()
    }
    if ($diagnostic) { [Console]::Error.Write($diagnostic) }
    try {
        $result = $text | ConvertFrom-Json
        if (!$result -or !$result.status -or !$result.summary) { throw 'Missing terminal result.' }
    } catch {
        [Console]::Error.WriteLine("browser-journey: $Name did not return a JSON result (exit $code).")
        if ($code -ne 0) { exit $code }
        exit 1
    }
    Write-Host ('{0,-12} {1,-10} {2}' -f $Name, $result.status, $result.summary)
    if ($code -ne 0 -or $result.status -ne 'succeeded') {
        [Console]::Error.WriteLine("browser-journey: stopped at $Name; inspect the result before any new call. No retry or cleanup was attempted.")
        if ($code -ne 0) { exit $code }
        exit 1
    }
    return $result
}

Write-Host "Ghostlight: $exe"
Write-Host ''
Write-Host ('{0,-12} {1,-10} {2}' -f 'STEP', 'STATUS', 'WHAT HAPPENED')
Write-Host ('{0,-12} {1,-10} {2}' -f '----', '------', '-------------')

$opened = Step 'open' 'browser_navigate' @{ url = $Url; new_tab = $true; reuse = 'never' }
$tab = $opened.facts.tab

# Every step from here names the tab this journey opened, so it never touches anything else of
# yours. A separate process each time, and the handle still resolves.
$null = Step 'list'       'browser_tabs'       @{ action = 'list' }
$null = Step 'read'       'browser_read'       @{ tab = $tab }
$null = Step 'screenshot' 'browser_screenshot' @{ tab = $tab } @('--output', $OutputPath)
$null = Step 'close'      'browser_tabs'       @{ action = 'close'; tab = $tab }

Write-Host ''
if (Test-Path -LiteralPath $OutputPath) {
    Write-Host "Screenshot: $OutputPath ($((Get-Item -LiteralPath $OutputPath -Force).Length) bytes)"
}

Write-Host 'Journey complete. Every step ran through ghostlight call, governed and audited as cli.'
exit 0
