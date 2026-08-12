#Requires -Version 7.0
<#
.SYNOPSIS
    The Sylin Card Foundry story, driven entirely by `ghostlight call`.

.DESCRIPTION
    All seven beats from docs/design/tcg-foundry-demo.md: a foil proof fails QA, Ghostlight inspects
    the defect, records a rejection, requests a revision, reads the page's own console and network
    evidence, attaches proof, completes the release packet, is refused when it tries to leave the
    domain, and finally hands the page an animated replay of its own work and erases the bytes.

    This shipped on the 0.8 line as the `ghostlight demo` subcommand. It does not need to be one:
    the command line reaches the same catalog through the same governance, so a recording operator
    retimes the story by editing a script rather than rebuilding a binary.

    Every step is its own process. They share a session because they share this shell (ADR-0106),
    which is what lets the target handles inventoried in beat two still resolve in beat seven.

    Nothing here captures your screen. `browser_record` is Ghostlight's own memory-only recording,
    which beat seven hands to the page and then erases.

.PARAMETER Url
    The demo stage. Defaults to the published Sylin stage.

.PARAMETER Ghostlight
    Path to the ghostlight executable. Defaults to PATH, then the repository build.

.PARAMETER Beat
    Seconds between actions, so each touched control gets a readable visual beat.

.PARAMETER Width
    Recording composition width. The design note frames the story at a 1280 x 720 page viewport, and
    a much larger viewport produces frames the GIF encoder refuses as unbounded.

.PARAMETER Height
    Recording composition height.

.PARAMETER KeepRecording
    Leave the recording in memory instead of discarding it at the end. It expires on its own.

.EXAMPLE
    ./scripts/demo-foundry.ps1
    ./scripts/demo-foundry.ps1 -Beat 0.6
#>
[CmdletBinding()]
param(
    [string] $Url = 'https://sylin.org/ghostlight/demo/foundry/',
    [string] $Ghostlight,
    [double] $Beat = 0.35,
    [int] $Width = 1280,
    [int] $Height = 800,
    [switch] $KeepRecording
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Rejection = 'Foil registration drifts past the lower-right safe area. Hold for Revision B.'
$Release = @{
    'Release name'  = 'Aurora Drop 01'
    'Set code'      = 'AUR-01'
    'Release owner' = 'Maya Chen'
    'QA note'       = 'Revision B clears the foil mask and the Sylin back stamp.'
}
$OffDomain = 'https://example.com/'

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
$shot = Join-Path ([System.IO.Path]::GetTempPath()) 'ghostlight-foundry-revision-b.jpg'

# Expect is the story's assertion: a beat that must be refused is as important as one that must
# succeed, and a demo that cannot tell them apart proves nothing.
function Step {
    param(
        [string] $Name,
        [string] $Tool,
        [hashtable] $Body = @{},
        [string[]] $Extra = @(),
        [string] $Expect = 'succeeded'
    )

    $result = & $exe call $Tool ($Body | ConvertTo-Json -Compress -Depth 6) --json @Extra | ConvertFrom-Json
    Write-Host ('{0,-16} {1,-10} {2}' -f $Name, $result.status, $result.summary)
    if ($result.status -ne $Expect) {
        throw "$Name expected $Expect but was $($result.status): $($result.summary)"
    }
    Start-Sleep -Seconds $Beat
    return $result
}

function Target {
    param($Items, [string] $Role, [string] $Name)
    $match = $Items | Where-Object { $_.role -eq $Role -and $_.name -like "$Name*" } | Select-Object -First 1
    if (-not $match) { throw "The stage exposes no $Role named '$Name'." }
    return $match.target
}

function Found {
    param([string] $Tab, [string] $Text, [string] $Role)
    $found = & $exe call browser_find (@{ tab = $Tab; text = $Text } | ConvertTo-Json -Compress) --json | ConvertFrom-Json
    $match = $found.facts.matches | Where-Object { $_.role -eq $Role } | Select-Object -First 1
    if (-not $match) { throw "The stage exposes no $Role matching '$Text'." }
    return $match.target
}

Write-Host "Ghostlight: $exe"
Write-Host "Stage:      $Url"
Write-Host ''
Write-Host ('{0,-16} {1,-10} {2}' -f 'BEAT', 'STATUS', 'WHAT HAPPENED')
Write-Host ('{0,-16} {1,-10} {2}' -f '----', '------', '-------------')

# 1. Open the Foundry, frame the composition, and start a memory-only recording lease.
# The frame is not decoration: recording is bounded, and a large viewport fails to encode.
$tab = (Step 'open' 'browser_navigate' @{ url = $Url; new_tab = $true }).facts.tab
$null = Step 'frame' 'browser_window' @{ tab = $tab; action = 'resize'; width = $Width; height = $Height }
# The whole story is recorded. Both bounds trade fidelity rather than coverage now: the extension
# thins retained frames at its byte bound, and the encoder thins again to fit the output bound.
$null = Step 'record start' 'browser_record' @{ action = 'start'; tab = $tab }

# 2. Inspect the workspace, hover the foil, rotate the card, zoom the defect.
$controls = (Step 'inspect' 'browser_inspect' @{ tab = $tab; scope = 'controls'; max_items = 200 }).facts.items
$rotate = Target $controls 'button' 'Rotate foil proof'
$null = Step 'hover foil' 'browser_hover' @{ tab = $tab; target = $rotate }
$null = Step 'rotate card' 'browser_click' @{ tab = $tab; target = $rotate }
$null = Step 'zoom defect' 'browser_window' @{ tab = $tab; action = 'zoom'; percent = 150 }
$null = Step 'zoom back' 'browser_window' @{ tab = $tab; action = 'zoom'; percent = 100 }

# 3. Record the failed criteria, explain the rejection, and move the ticket.
$null = Step 'qa drift' 'browser_click' @{ tab = $tab; target = (Target $controls 'checkbox' 'Foil registration drift') }
$null = Step 'qa safe-area' 'browser_click' @{ tab = $tab; target = (Target $controls 'checkbox' 'Border safe-area collision') }
$null = Step 'reason' 'browser_type_text' @{ tab = $tab; target = (Target $controls 'textbox' 'Rejection reason'); text = $Rejection }
$null = Step 'drag ticket' 'browser_drag' @{
    tab                = $tab
    source_target      = (Target $controls 'button' 'Drag QA-017 defect ticket')
    destination_target = (Found $tab 'Request revision' 'span')
}

# 4. Read the page's own console and network evidence, then wait for the corrected proof.
$null = Step 'diagnose' 'browser_diagnose' @{ tab = $tab; source = 'both'; detail = 'all'; limit = 20 }
$null = Step 'await rev B' 'browser_wait' @{ tab = $tab; condition = 'text_present'; value = 'Revision B ready' }


# 5. Capture the corrected proof, attach it, finish the QA checks, and complete the packet.
$null = Step 'capture' 'browser_screenshot' @{ tab = $tab } @('--output', $shot)
if (-not (Test-Path -LiteralPath $shot)) { throw "No screenshot was written to $shot." }
$after = (Step 're-inspect' 'browser_inspect' @{ tab = $tab; scope = 'controls'; max_items = 200 }).facts.items
$null = Step 'attach proof' 'browser_upload' @{
    tab = $tab; target = (Target $after 'textbox' 'Revision B screenshot evidence'); paths = @($shot)
}
foreach ($check in @('Foil registration verified', 'Sylin back stamp verified', 'Visual evidence attached')) {
    $null = Step "qa $($check.Split(' ')[0].ToLower())" 'browser_click' @{ tab = $tab; target = (Target $after 'checkbox' $check) }
}
$null = Step 'release packet' 'browser_fill_form' @{
    tab    = $tab
    fields = @($Release.GetEnumerator() | ForEach-Object { @{ target = (Target $after 'textbox' $_.Key); value = $_.Value } })
}
$null = Step 'complete' 'browser_click' @{ tab = $tab; target = (Target $after 'button' 'Complete release packet') }

# 6. Try to leave the domain. The refusal is the point, so anything else fails the run.
$null = Step 'off-domain' 'browser_navigate' @{
    tab = $tab; url = $OffDomain; restrict_hosts = @('sylin.org')
} @() 'blocked'

# 7. Hand the page the replay, confirm it landed, and erase the bytes.
$null = Step 'save replay' 'browser_record' @{
    action = 'save'; target = (Target $after 'textbox' 'Animated Ghostlight replay')
}
$null = Step 'replay landed' 'browser_wait' @{ tab = $tab; condition = 'text_present'; value = 'Replay ready' }
if (-not $KeepRecording) {
    $null = Step 'erase bytes' 'browser_record' @{ action = 'discard' }
}

Remove-Item -LiteralPath $shot -Force -ErrorAction SilentlyContinue
Write-Host ''
Write-Host 'Story complete: inspected, rejected, revised, evidenced, refused off-domain, replayed, erased.'
Write-Host 'The tab stays open for your capture; close it when you are done.'
exit 0
