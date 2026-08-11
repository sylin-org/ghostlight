#Requires -Version 7.0
<#
.SYNOPSIS
    A complete governed browser journey driven entirely by `ghostlight call`.

.DESCRIPTION
    No MCP client and no model: every step is a command-line invocation against the local Ghostlight
    authority, which governs it exactly as it governs an agent's call and audits it under the `cli`
    channel.

    The journey holds one session open and writes a line at a time. That matters, because tab,
    target, and view handles belong to a session and a session lasts as long as the process: two
    separate `ghostlight call` commands could not share the tab this opens. Keeping the process open
    also means each step can read the previous step's result and use the handle it returned, which
    is how `browser_close_tab` closes exactly the tab this script opened and nothing else.

.PARAMETER Url
    The page to open. Defaults to example.com.

.PARAMETER Ghostlight
    Path to the ghostlight executable. Defaults to PATH, then the repository debug build.

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
    param([string] $Explicit)

    if ($Explicit) {
        if (-not (Test-Path -LiteralPath $Explicit)) { throw "No ghostlight executable at $Explicit" }
        return (Resolve-Path -LiteralPath $Explicit).Path
    }
    $onPath = Get-Command 'ghostlight' -CommandType Application -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }

    $suffix = if ($IsWindows) { '.exe' } else { '' }
    $repository = Split-Path -Parent $PSScriptRoot
    foreach ($target in @('.target-ghostlight-1.0', 'target')) {
        $candidate = Join-Path $repository (Join-Path $target (Join-Path 'debug' "ghostlight$suffix"))
        if (Test-Path -LiteralPath $candidate) { return (Resolve-Path -LiteralPath $candidate).Path }
    }
    throw 'Could not find ghostlight. Put it on PATH or pass -Ghostlight.'
}

# One long-lived `ghostlight call --stdin` process is the session. Standard error is left attached
# to the console on purpose: redirecting a stream nobody drains can fill its buffer and deadlock.
function Start-GhostlightSession {
    param([string] $Exe, [string] $CapturePath)

    $start = [System.Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Exe
    foreach ($argument in @('call', '--stdin', '--json')) { $start.ArgumentList.Add($argument) }
    if ($CapturePath) {
        $start.ArgumentList.Add('--output')
        $start.ArgumentList.Add($CapturePath)
    }
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.UseShellExecute = $false
    return [System.Diagnostics.Process]::Start($start)
}

# Write one call, read its one terminal result. The orchestrator answers in order, so a script can
# use what it just learned in the very next line.
function Invoke-GhostlightStep {
    param(
        [System.Diagnostics.Process] $Session,
        [string] $Tool,
        [hashtable] $Body = @{}
    )

    $json = $Body | ConvertTo-Json -Compress
    $Session.StandardInput.WriteLine("$Tool $json")
    $Session.StandardInput.Flush()
    $line = $Session.StandardOutput.ReadLine()
    if (-not $line) { throw "Ghostlight closed the session during $Tool" }
    return $line | ConvertFrom-Json
}

$exe = Resolve-Ghostlight -Explicit $Ghostlight
Write-Host "Ghostlight: $exe"

# The catalog proves the service is reachable, and demand-starts it when it is not running.
$catalog = & $exe call --catalog
if ($LASTEXITCODE -ne 0) { throw "Ghostlight is not reachable (exit $LASTEXITCODE)" }
Write-Host "Catalog:    $($catalog.Count) tools"
Write-Host ''
Write-Host ('{0,-12} {1,-10} {2}' -f 'STEP', 'STATUS', 'WHAT HAPPENED')
Write-Host ('{0,-12} {1,-10} {2}' -f '----', '------', '-------------')

$failed = [System.Collections.Generic.List[string]]::new()
$session = Start-GhostlightSession -Exe $exe -CapturePath $OutputPath

function Step {
    param([string] $Name, [string] $Tool, [hashtable] $Body = @{})

    $result = Invoke-GhostlightStep -Session $session -Tool $Tool -Body $Body
    Write-Host ('{0,-12} {1,-10} {2}' -f $Name, $result.status, $result.summary)
    if ($result.status -ne 'succeeded') { $failed.Add($Name) }
    return $result
}

try {
    $opened = Step 'open' 'browser_open_page' @{ url = $Url }
    # Everything after this uses the handle the open returned, so the journey acts only on the tab
    # it created, never on whatever else the user has open.
    $tab = if ($opened.status -eq 'succeeded') { $opened.facts.tab } else { $null }
    if (-not $tab) { throw "Could not open $Url : $($opened.summary)" }

    $null = Step 'list'       'browser_list_tabs'
    $null = Step 'read'       'browser_read_page'       @{ tab = $tab }
    $null = Step 'screenshot' 'browser_take_screenshot' @{ tab = $tab }
    $null = Step 'close'      'browser_close_tab'       @{ tab = $tab }
} finally {
    $session.StandardInput.Close()
    $null = $session.WaitForExit(10000)
}

$sessionExit = $session.ExitCode

Write-Host ''
if (Test-Path -LiteralPath $OutputPath) {
    $size = (Get-Item -LiteralPath $OutputPath).Length
    Write-Host "Screenshot: $OutputPath ($size bytes)"
} else {
    Write-Host 'Screenshot: not written'
    $failed.Add('screenshot-file')
}

if ($failed.Count -gt 0) {
    Write-Error ("Journey failed at: {0}" -f ($failed -join ', '))
    exit 1
}
if ($sessionExit -ne 0) {
    Write-Error "Journey session exited $sessionExit"
    exit $sessionExit
}

Write-Host 'Journey complete. Every step ran through ghostlight call, governed and audited as cli.'
exit 0
