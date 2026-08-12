#Requires -Version 7.0
<#
.SYNOPSIS
    The launch-brief demo story, driven entirely by `ghostlight call`.

.DESCRIPTION
    This is the story specified in docs/design/demo-brief.md: read the page once, fill three fields
    as separately paced writes, tick two boxes, submit, and hold the completion state. It was
    designed as a Rust subcommand (`ghostlight demo-brief`) and never implemented. It does not need
    to be one -- the command line reaches the same catalog through the same governance, and a
    recording operator can retime the story by editing a script instead of rebuilding a binary.

    Every step is its own process. They share a session because they share this shell (ADR-0106),
    which is what lets the target handles inventoried in step three still resolve in step nine.

    The pacing defaults are the ones the design note tuned for a short capture. Nothing here records
    anything: run your desktop capture against the visible Chrome window.

.PARAMETER Url
    The demo stage. Defaults to the published Sylin stage.

.PARAMETER Ghostlight
    Path to the ghostlight executable. Defaults to PATH, then the repository build.

.PARAMETER SetupHold
    Seconds to hold the loaded page before work begins. Editors usually trim this.

.PARAMETER ScanHold
    Seconds to hold after the page scan.

.PARAMETER Beat
    Seconds between actions, so each touched control gets a readable visual beat.

.PARAMETER CompletionHold
    Seconds to hold the finished state on screen.

.EXAMPLE
    ./scripts/demo-brief.ps1
    ./scripts/demo-brief.ps1 -Beat 0.4 -CompletionHold 5
#>
[CmdletBinding()]
param(
    [string] $Url = 'https://sylin.org/ghostlight/demo/brief/',
    [string] $Ghostlight,
    [double] $SetupHold = 2.0,
    [double] $ScanHold = 1.6,
    [double] $Beat = 0.25,
    [double] $CompletionHold = 3.0
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# The fictional values are stable, so every recording tells the same story.
$Project = 'Moonlight Notes'
$Owner = 'Maya Chen'
$Summary = 'Turn field observations into a shared release brief.'
$Completion = "$Project is ready for review."

function Resolve-Ghostlight {
    if ($Ghostlight) { return (Resolve-Path -LiteralPath $Ghostlight).Path }
    $found = Get-Command 'ghostlight' -CommandType Application -ErrorAction SilentlyContinue
    if ($found) { return $found.Source }

    $suffix = if ($IsWindows) { '.exe' } else { '' }
    $repository = Split-Path -Parent $PSScriptRoot
    foreach ($build in @('target/release', '.target-ghostlight-1.0/debug', 'target/debug')) {
        $candidate = Join-Path $repository "$build/ghostlight$suffix"
        if (Test-Path -LiteralPath $candidate) { return (Resolve-Path -LiteralPath $candidate).Path }
    }
    throw 'Could not find ghostlight. Put it on PATH or pass -Ghostlight.'
}

$exe = Resolve-Ghostlight
$worst = 0

function Step {
    param([string] $Name, [string] $Tool, [hashtable] $Body = @{})

    $result = & $exe call $Tool ($Body | ConvertTo-Json -Compress) --json | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { $script:worst = $LASTEXITCODE }
    Write-Host ('{0,-14} {1,-10} {2}' -f $Name, $result.status, $result.summary)
    if ($result.status -ne 'succeeded') { throw "$Name did not succeed: $($result.summary)" }
    return $result
}

# The accessible names carry their helper text, so a checkbox is matched by its leading phrase.
function Find-Target {
    param($Items, [string] $Role, [string] $Name)

    $match = $Items | Where-Object { $_.role -eq $Role -and $_.name -like "$Name*" } | Select-Object -First 1
    if (-not $match) { throw "The stage exposes no $Role named '$Name'." }
    return $match.target
}

Write-Host "Ghostlight: $exe"
Write-Host "Stage:      $Url"
Write-Host ''
Write-Host ('{0,-14} {1,-10} {2}' -f 'STEP', 'STATUS', 'WHAT HAPPENED')
Write-Host ('{0,-14} {1,-10} {2}' -f '----', '------', '-------------')

$opened = Step 'open' 'browser_navigate' @{ url = $Url }
$tab = $opened.facts.tab
Start-Sleep -Seconds $SetupHold

# One read establishes that the agent understands the surface before it touches anything.
$null = Step 'scan' 'browser_read' @{ tab = $tab }
Start-Sleep -Seconds $ScanHold

# One inventory, then reuse: the handles stay good while the document does.
$controls = (Step 'inventory' 'browser_inspect' @{ tab = $tab; scope = 'controls' }).facts.items
$fields = @{
    project = Find-Target $controls 'textbox' 'Project'
    owner   = Find-Target $controls 'textbox' 'Owner'
    summary = Find-Target $controls 'textbox' 'Summary'
}
$boxes = @{
    screenshots = Find-Target $controls 'checkbox' 'Include screenshots'
    local       = Find-Target $controls 'checkbox' 'Keep data local'
}
$submit = Find-Target $controls 'button' 'Create brief'

# Three separate writes rather than one form fill: each visible field gets its own beat, which is a
# recording decision. An agent that does not need pacing should prefer one browser_fill_form call.
foreach ($field in @(
    @{ Name = 'field project'; Target = $fields.project; Text = $Project }
    @{ Name = 'field owner';   Target = $fields.owner;   Text = $Owner }
    @{ Name = 'field summary'; Target = $fields.summary; Text = $Summary }
)) {
    $null = Step $field.Name 'browser_type_text' @{ tab = $tab; target = $field.Target; text = $field.Text }
    Start-Sleep -Seconds $Beat
}

$null = Step 'tick shots' 'browser_click' @{ tab = $tab; target = $boxes.screenshots }
Start-Sleep -Seconds $Beat
$null = Step 'tick local' 'browser_click' @{ tab = $tab; target = $boxes.local }
Start-Sleep -Seconds $Beat

# Submission stays its own step so the intent is visible to a viewer.
$null = Step 'submit' 'browser_click' @{ tab = $tab; target = $submit }

# Wait for the exact sentence rather than a fixed sleep, so the hold starts when the page is ready.
$null = Step 'completion' 'browser_wait' @{ tab = $tab; condition = 'text_present'; value = $Completion }
Start-Sleep -Seconds $CompletionHold

Write-Host ''
Write-Host "Story complete. The tab stays open for your capture; close it when you are done."
exit $worst
