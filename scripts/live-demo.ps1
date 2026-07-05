<#
.SYNOPSIS
  Live end-to-end demo / smoke test for Ghostlight (Windows).

.DESCRIPTION
  Spawns the ghostlight mcp-server, speaks MCP (newline-delimited JSON-RPC) to it
  over stdio, and drives the REAL browser extension through a scripted sequence
  with timed pauses so you can watch it happen: the ghost tab group, the sky-blue
  cursor and glow, click ripples (1/2/3), the dashed right-click ring, the drag
  comet-trail, and the type shimmer -- plus tools/list, explain, read_page,
  get_page_text and screenshot for coverage.

  Exits non-zero if any tool call errors, so it doubles as a manual end-to-end
  smoke test. The visual effects are hidden from screenshots by design, so this is
  a watch-your-browser demo: keep the browser window visible while it runs.

  Targeting is rescale-proof: it opens a FRESH tab (no leftover screenshot
  context, so drag coordinates pass through 1:1) and clicks by element ref (refs
  are never rescaled), rather than by raw coordinate.

.PARAMETER Exe
  Path to ghostlight.exe. Defaults to target\debug\ghostlight.exe (or
  target\release with -Release), resolved relative to this script.

.PARAMETER Pause
  Seconds to pause after each visible step so you can watch it. Default 2.

.PARAMETER Release
  Use the release build instead of the debug build.

.PARAMETER TimeoutSec
  How long to wait for any single server response before giving up. Default 30
  (covers the first-call extension warmup wait).

.PREREQUISITES
  - The Ghostlight extension is loaded and enabled in Chrome (chrome://extensions).
  - No other ghostlight mcp-server is running: close your Claude Code ghostlight
    connection first. This demo must own the IPC endpoint; it checks and aborts
    with a friendly message if the endpoint is already taken.
  - The binary is built: run "cargo build" (or "cargo build --release" with -Release).

.EXAMPLE
  pwsh -File scripts\live-demo.ps1

.EXAMPLE
  pwsh -File scripts\live-demo.ps1 -Pause 3 -Release
#>
param(
  [string]$Exe,
  [double]$Pause = 2,
  [switch]$Release,
  [int]$TimeoutSec = 30
)

$ErrorActionPreference = 'Stop'

$ENDPOINT_PIPE = '\\.\pipe\org.sylin.ghostlight.v1'
$Ghost = [char]::ConvertFromUtf32(0x1F47B) # the ghost emoji, built from its codepoint so this file stays ASCII
$script:ReadTimeoutMs = [int]($TimeoutSec * 1000)
$script:rpcId = 0
$script:writer = $null
$script:reader = $null

# --- page scripts injected via javascript_tool -------------------------------

$PLAYGROUND_JS = @'
const old=document.getElementById('fx-demo'); if(old) old.remove();
const d=document.createElement('div');
d.id='fx-demo';
d.addEventListener('contextmenu', function(e){ e.preventDefault(); }); // right-click shows the ring, not the native menu
d.style.cssText='position:fixed;inset:0;z-index:2147483000;background:linear-gradient(160deg,#f8fafc,#e7edf5);font-family:system-ui,-apple-system,sans-serif;color:#0f172a;display:flex;flex-direction:column;align-items:center;gap:26px;padding-top:56px;box-sizing:border-box';
d.innerHTML='<h1 style="margin:0;font-size:30px;font-weight:700">Ghostlight FX playground</h1>'+
'<button id="fx-btn" style="width:380px;height:92px;font-size:22px;font-weight:600;border:none;border-radius:14px;background:#e2e8f0;color:#0f172a;cursor:pointer;box-shadow:0 2px 8px rgba(0,0,0,.08)">click / double / triple / right-click me</button>'+
'<input id="fx-input" placeholder="the agent will type here" style="width:380px;height:64px;font-size:20px;padding:0 18px;border:2px solid #cbd5e1;border-radius:14px;box-sizing:border-box;outline:none" />'+
'<div id="fx-drag" style="width:640px;height:170px;border:2px dashed #94a3b8;border-radius:16px;display:flex;align-items:center;justify-content:center;font-size:18px;color:#475569">drag across me</div>';
document.body.appendChild(d);
const g=document.getElementById('fx-drag').getBoundingClientRect();
JSON.stringify({ dragA: [Math.round(g.left+50), Math.round(g.top+g.height/2)], dragB: [Math.round(g.right-50), Math.round(g.top+g.height/2)] })
'@

# --- MCP client helpers ------------------------------------------------------

# Send one JSON-RPC request and return the parsed response (skipping notifications
# and unrelated ids). Use -Notify for a notification (no id, no response awaited).
function Send-Rpc {
  param([string]$Method, $Params, [switch]$Notify)
  $obj = [ordered]@{ jsonrpc = '2.0'; method = $Method }
  if ($null -ne $Params) { $obj.params = $Params }
  if (-not $Notify) { $script:rpcId++; $obj.id = $script:rpcId }
  $script:writer.WriteLine(($obj | ConvertTo-Json -Compress -Depth 20))
  if ($Notify) { return }
  $targetId = $obj.id
  while ($true) {
    $task = $script:reader.ReadLineAsync()
    if (-not $task.Wait($script:ReadTimeoutMs)) { throw "Timed out ($($script:ReadTimeoutMs) ms) waiting for a response to '$Method'." }
    $line = $task.Result
    if ($null -eq $line) { throw "Server closed its output stream while awaiting '$Method'." }
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $msg = $line | ConvertFrom-Json
    if ($null -ne $msg.PSObject.Properties['id'] -and $msg.id -eq $targetId) { return $msg }
    # otherwise a notification (e.g. tools/list_changed) or unrelated id -- keep reading
  }
}

# Call a tool and return its result object, throwing on a protocol error or a tool isError.
function Invoke-Tool {
  param([string]$Name, $Arguments)
  $resp = Send-Rpc -Method 'tools/call' -Params @{ name = $Name; arguments = $Arguments }
  if ($null -ne $resp.PSObject.Properties['error']) { throw "tools/call '$Name' returned JSON-RPC error: $($resp.error.message)" }
  $result = $resp.result
  if ($null -ne $result.PSObject.Properties['isError'] -and $result.isError) { throw "tool '$Name' reported isError: $(Get-ResultText $result)" }
  return $result
}

# The first text block of a tool result (tool results are { content: [ { type, text } ] }).
function Get-ResultText {
  param($Result)
  if ($null -eq $Result) { return '' }
  $item = $Result.content | Where-Object { $_.type -eq 'text' } | Select-Object -First 1
  if ($null -eq $item) { return '' }
  return $item.text
}

# Run a computer action and echo where it actually landed (so a mis-target is visible).
function Invoke-Computer {
  param($Arguments)
  Write-Host "   $(Get-ResultText (Invoke-Tool 'computer' $Arguments))"
}

# Pull an element ref (ref_N) out of read_page output by role keyword (e.g. 'button', 'textbox').
function Get-Ref {
  param([string]$Text, [string]$Role)
  foreach ($line in ($Text -split "`n")) {
    if ($line -match "\b$Role\b" -and $line -match '\[(ref_\d+)\]') { return $Matches[1] }
  }
  throw "Could not find a '$Role' ref in read_page output."
}

function Step {
  param([string]$Message)
  Write-Host ''
  Write-Host ">> $Message" -ForegroundColor Cyan
}

# --- resolve the binary ------------------------------------------------------

if (-not $Exe) {
  $rel = if ($Release) { 'target\release\ghostlight.exe' } else { 'target\debug\ghostlight.exe' }
  $Exe = Join-Path $PSScriptRoot (Join-Path '..' $rel)
}
if (-not (Test-Path $Exe)) {
  Write-Host "ghostlight.exe not found at: $Exe" -ForegroundColor Red
  Write-Host "Build it first: cargo build$(if ($Release) { ' --release' })" -ForegroundColor Red
  exit 1
}
$Exe = (Resolve-Path $Exe).Path

# --- pre-flight: the demo must own the IPC endpoint --------------------------

if (Test-Path $ENDPOINT_PIPE) {
  Write-Host "A ghostlight mcp-server already owns $ENDPOINT_PIPE." -ForegroundColor Yellow
  Write-Host "Is your Claude Code ghostlight connection open? Close it first -- this demo needs to own" -ForegroundColor Yellow
  Write-Host "the endpoint, and a second server would just be refused (single active session). Aborting." -ForegroundColor Yellow
  exit 1
}

# --- run ---------------------------------------------------------------------

Write-Host "Ghostlight live demo" -ForegroundColor White
Write-Host "  binary : $Exe"
Write-Host "  pause  : $Pause s between steps"
Write-Host "  NOTE   : keep the browser visible -- effects are hidden from screenshots by design."

$proc = $null
$exitCode = 1
try {
  $psi = New-Object System.Diagnostics.ProcessStartInfo
  $psi.FileName = $Exe
  $psi.RedirectStandardInput = $true
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $psi.UseShellExecute = $false
  $psi.CreateNoWindow = $true
  $proc = [System.Diagnostics.Process]::Start($psi)
  $script:writer = $proc.StandardInput
  $script:reader = $proc.StandardOutput
  $script:writer.NewLine = "`n"
  $script:writer.AutoFlush = $true

  Step 'Handshake (initialize)'
  Send-Rpc -Method 'initialize' -Params @{ protocolVersion = '2024-11-05'; capabilities = @{}; clientInfo = @{ name = 'ghostlight-live-demo'; version = '1' } } | Out-Null
  Send-Rpc -Method 'notifications/initialized' -Notify

  Step 'tools/list'
  $tl = Send-Rpc -Method 'tools/list' -Params @{}
  Write-Host "   advertised $($tl.result.tools.Count) tools (13 trained + explain)"

  Step "Open the $($Ghost)Ghostlight group, then a fresh tab in it"
  Invoke-Tool 'tabs_context_mcp' @{ createIfEmpty = $true } | Out-Null
  $created = Get-ResultText (Invoke-Tool 'tabs_create_mcp' @{})
  if ($created -notmatch 'Created tab (\d+)') { throw "Could not parse the new tab id from: $created" }
  $tabId = [long]$Matches[1]
  # A fresh tab has no screenshot context, so drag coordinates pass through 1:1.
  Write-Host "   tabId = $tabId  (watch a new window with the $($Ghost)Ghostlight group)"
  Start-Sleep -Seconds $Pause

  Step 'Navigate to example.com'
  Invoke-Tool 'navigate' @{ tabId = $tabId; url = 'https://example.com/' } | Out-Null
  Start-Sleep -Seconds $Pause

  Step 'Inject a small FX playground'
  $drag = Get-ResultText (Invoke-Tool 'javascript_tool' @{ action = 'javascript_exec'; tabId = $tabId; text = $PLAYGROUND_JS }) | ConvertFrom-Json
  $page = Get-ResultText (Invoke-Tool 'read_page' @{ tabId = $tabId; filter = 'interactive' })
  $btnRef = Get-Ref $page 'button'
  $inputRef = Get-Ref $page 'textbox'
  Write-Host "   button=$btnRef  input=$inputRef  (clicking by ref -- never rescaled)"
  Start-Sleep -Seconds $Pause

  Step 'explain: the capability directory'
  Write-Host (Get-ResultText (Invoke-Tool 'explain' @{}))
  Start-Sleep -Seconds $Pause

  Step 'Single click -> one ripple'
  Invoke-Computer @{ action = 'left_click'; tabId = $tabId; ref = $btnRef }
  Start-Sleep -Seconds $Pause

  Step 'Double click -> two ripples'
  Invoke-Computer @{ action = 'double_click'; tabId = $tabId; ref = $btnRef }
  Start-Sleep -Seconds $Pause

  Step 'Triple click -> three ripples'
  Invoke-Computer @{ action = 'triple_click'; tabId = $tabId; ref = $btnRef }
  Start-Sleep -Seconds $Pause

  Step 'Right click -> the dashed ring'
  Invoke-Computer @{ action = 'right_click'; tabId = $tabId; ref = $btnRef }
  Start-Sleep -Seconds $Pause

  Step 'Drag -> the comet trail'
  Invoke-Computer @{ action = 'left_click_drag'; tabId = $tabId; start_coordinate = $drag.dragA; coordinate = $drag.dragB }
  Start-Sleep -Seconds $Pause

  Step 'Click the field, then type -> ripple + shimmer'
  Invoke-Computer @{ action = 'left_click'; tabId = $tabId; ref = $inputRef }
  Start-Sleep -Milliseconds 500
  Invoke-Computer @{ action = 'type'; tabId = $tabId; text = 'Ghostlight is watching the stage.' }
  Start-Sleep -Seconds $Pause

  Step 'read_page / get_page_text / screenshot (coverage)'
  $refCount = (($page -split "`n") | Where-Object { $_ -match '\[ref_' }).Count
  Write-Host "   read_page (interactive): $refCount referenced element(s)"
  $pt = Get-ResultText (Invoke-Tool 'get_page_text' @{ tabId = $tabId })
  Write-Host "   get_page_text: $($pt.Length) chars"
  Invoke-Tool 'computer' @{ action = 'screenshot'; tabId = $tabId } | Out-Null
  Write-Host '   screenshot captured (effects are hidden in captures by design)'

  Write-Host ''
  Write-Host 'Demo complete -- every tool call succeeded.' -ForegroundColor Green
  Write-Host 'Covered live: tabs_context, tabs_create, navigate, javascript_tool, explain, computer'
  Write-Host '  (left_click / double / triple / right / drag / type / screenshot), read_page, get_page_text.'
  $exitCode = 0
}
catch {
  Write-Host ''
  Write-Host "DEMO FAILED: $($_.Exception.Message)" -ForegroundColor Red
  $exitCode = 1
}
finally {
  try { if ($null -ne $script:writer) { $script:writer.Close() } } catch {}
  $stderr = ''
  try { if ($null -ne $proc) { $stderr = $proc.StandardError.ReadToEnd() } } catch {}
  try { if ($null -ne $proc -and -not $proc.WaitForExit(3000)) { $proc.Kill() } } catch {}
  if ($exitCode -ne 0 -and $stderr) {
    Write-Host '--- server stderr ---' -ForegroundColor DarkGray
    Write-Host $stderr -ForegroundColor DarkGray
  }
}

exit $exitCode
