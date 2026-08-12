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
$worst = 0

# One call, reported as a row. The exit code comes from Ghostlight rather than being invented here,
# so a governed refusal (2) stays distinguishable from a failure (4) and an uncertain effect (6).
function Step {
    param([string] $Name, [string] $Tool, [hashtable] $Body = @{}, [string[]] $Extra = @())

    $result = & $exe call $Tool ($Body | ConvertTo-Json -Compress) --json @Extra | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { $script:worst = $LASTEXITCODE }
    Write-Host ('{0,-12} {1,-10} {2}' -f $Name, $result.status, $result.summary)
    return $result
}

Write-Host "Ghostlight: $exe"
Write-Host ''
Write-Host ('{0,-12} {1,-10} {2}' -f 'STEP', 'STATUS', 'WHAT HAPPENED')
Write-Host ('{0,-12} {1,-10} {2}' -f '----', '------', '-------------')

$opened = Step 'open' 'browser_navigate' @{ url = $Url }
if ($opened.status -ne 'succeeded') { throw "Could not open $Url : $($opened.summary)" }
$tab = $opened.facts.tab

# Every step from here names the tab this journey opened, so it never touches anything else of
# yours. A separate process each time, and the handle still resolves.
$null = Step 'list'       'browser_tabs'       @{ action = 'list' }
$null = Step 'read'       'browser_read'       @{ tab = $tab }
$null = Step 'screenshot' 'browser_screenshot' @{ tab = $tab } @('--output', $OutputPath)
$null = Step 'close'      'browser_tabs'       @{ action = 'close'; tab = $tab }

Write-Host ''
if (Test-Path -LiteralPath $OutputPath) {
    Write-Host "Screenshot: $OutputPath ($((Get-Item -LiteralPath $OutputPath).Length) bytes)"
}

switch ($worst) {
    0 { Write-Host 'Journey complete. Every step ran through ghostlight call, governed and audited as cli.' }
    2 { Write-Host 'Journey finished with a governed refusal. That is Ghostlight working, not failing.' }
    default { Write-Host "Journey did not complete cleanly (exit $worst)." }
}
exit $worst
