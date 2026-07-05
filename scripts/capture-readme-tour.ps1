<#
.SYNOPSIS
  Scripted, replayable README tour of the Ghostlight repo, for recording a self-narrating demo GIF.

.DESCRIPTION
  Spawns the ghostlight mcp-server, speaks MCP over stdio, and drives the REAL browser extension
  through a fixed sequence with timed pauses: open the README, toggle to Code, browse
  docs -> guides -> solo-developer.md in the file tree, scroll, and return home to close the loop.
  Every step fires the per-action visual feedback (nav pill, ripple + target-glow, scroll chevrons,
  scan-line, ...) so a screen recording narrates itself.

  Targeting is by fixed viewport coordinate (harvested once at 1280x800). The window is resized to
  1280x800 so those coordinates are stable on every replay, and no screenshot is taken before the
  clicks, so coordinates pass through 1:1 (no rescale). If GitHub shifts its layout, adjust the
  coordinate block below -- it is the only thing that changes.

.PREREQUISITES
  - The Ghostlight extension is loaded and enabled in Chrome (chrome://extensions).
  - No other ghostlight mcp-server owns the endpoint: close your Claude Code ghostlight connection
    first (this script must own the IPC endpoint; it aborts with a friendly message otherwise).
  - The binary is built: "cargo build" (or "cargo build --release" with -Release).
  - To capture the subtitle track too, turn on "Show action captions" in the extension popup first.

.PARAMETER RecordDelay
  Seconds to wait after the README loads before the tour begins, so you can start your recorder.
  Default 10.

.PARAMETER StepPause
  Seconds to hold after each visible step. Default 2.

.EXAMPLE
  pwsh -File scripts\capture-readme-tour.ps1

.EXAMPLE
  pwsh -File scripts\capture-readme-tour.ps1 -Release -RecordDelay 8
#>
param(
  [string]$Exe,
  [switch]$Release,
  [double]$RecordDelay = 10,
  [double]$StepPause = 2,
  [int]$TimeoutSec = 30
)

$ErrorActionPreference = 'Stop'

# --- Fixed targets (viewport coordinates at 1280x800). Adjust here if GitHub's layout shifts. ---
$WIN = @{ w = 1280; h = 800 }
$README = 'https://github.com/sylin-org/ghostlight/blob/main/README.md'
$T = @{
  Code   = @(451, 249)   # the Preview | Code | Blame toggle, on the README blob
  Docs   = @(77, 296)    # "docs" folder in the left file tree
  Guides = @(91, 425)    # "guides" folder, once docs is expanded
  Solo   = @(137, 522)   # "solo-developer.md", once guides is expanded
  ScrollAt = @(780, 400) # a point inside the article, to scroll it
  Home   = @(386, 25)    # the "ghostlight" breadcrumb -> repo root (README), closing the loop
}

$ENDPOINT_PIPE = '\\.\pipe\org.sylin.ghostlight.v1'
$script:ReadTimeoutMs = [int]($TimeoutSec * 1000)
$script:rpcId = 0
$script:writer = $null
$script:reader = $null

# --- MCP client helpers (same harness as scripts\live-demo.ps1) ---

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
    if (-not $task.Wait($script:ReadTimeoutMs)) { throw "Timed out waiting for a response to '$Method'." }
    $line = $task.Result
    if ($null -eq $line) { throw "Server closed its output stream while awaiting '$Method'." }
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $msg = $line | ConvertFrom-Json
    if ($null -ne $msg.PSObject.Properties['id'] -and $msg.id -eq $targetId) { return $msg }
  }
}

function Invoke-Tool {
  param([string]$Name, $Arguments)
  $resp = Send-Rpc -Method 'tools/call' -Params @{ name = $Name; arguments = $Arguments }
  if ($null -ne $resp.PSObject.Properties['error']) { throw "tools/call '$Name' JSON-RPC error: $($resp.error.message)" }
  $result = $resp.result
  if ($null -ne $result.PSObject.Properties['isError'] -and $result.isError) {
    $t = ($result.content | Where-Object { $_.type -eq 'text' } | Select-Object -First 1)
    throw "tool '$Name' reported isError: $($t.text)"
  }
  return $result
}

function Get-ResultText {
  param($Result)
  if ($null -eq $Result) { return '' }
  $item = $Result.content | Where-Object { $_.type -eq 'text' } | Select-Object -First 1
  if ($null -eq $item) { return '' }
  return $item.text
}

function Click {
  param([int[]]$Xy)
  Invoke-Tool 'computer' @{ action = 'left_click'; tabId = $script:tabId; coordinate = $Xy } | Out-Null
}

function Step {
  param([string]$Message)
  Write-Host ">> $Message" -ForegroundColor Cyan
}

# --- resolve the binary ---

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

if (Test-Path $ENDPOINT_PIPE) {
  Write-Host "A ghostlight mcp-server already owns $ENDPOINT_PIPE." -ForegroundColor Yellow
  Write-Host "Close your Claude Code ghostlight connection first -- this tour must own the endpoint. Aborting." -ForegroundColor Yellow
  exit 1
}

# --- run ---

Write-Host "Ghostlight README tour" -ForegroundColor White
Write-Host "  binary : $Exe"
Write-Host "  record : $RecordDelay s to start your recorder after the README loads"

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

  Send-Rpc -Method 'initialize' -Params @{ protocolVersion = '2024-11-05'; capabilities = @{}; clientInfo = @{ name = 'ghostlight-readme-tour'; version = '1' } } | Out-Null
  Send-Rpc -Method 'notifications/initialized' -Notify

  Step "Open the ghost tab group and a fresh tab in it"
  Invoke-Tool 'tabs_context_mcp' @{ createIfEmpty = $true } | Out-Null
  $created = Get-ResultText (Invoke-Tool 'tabs_create_mcp' @{})
  if ($created -notmatch 'Created tab (\d+)') { throw "Could not parse the new tab id from: $created" }
  $script:tabId = [long]$Matches[1]

  Step "Resize to $($WIN.w)x$($WIN.h) so the coordinates are stable"
  Invoke-Tool 'resize_window' @{ width = $WIN.w; height = $WIN.h; tabId = $script:tabId } | Out-Null

  Step "Navigate to the README (destination pill)"
  Invoke-Tool 'navigate' @{ tabId = $script:tabId; url = $README } | Out-Null

  Write-Host ""
  Write-Host "START YOUR RECORDING NOW -- the tour begins in $RecordDelay s" -ForegroundColor Green
  for ($i = [int]$RecordDelay; $i -gt 0; $i--) { Write-Host "  $i..." -NoNewline; Start-Sleep -Seconds 1 }
  Write-Host ""

  Step "Toggle to Code (ripple + the toggle glows)";           Click $T.Code;   Start-Sleep -Seconds $StepPause
  Step "Open the docs folder in the tree";                     Click $T.Docs;   Start-Sleep -Seconds $StepPause
  Step "Open guides";                                          Click $T.Guides; Start-Sleep -Seconds $StepPause
  Step "Open solo-developer.md";                               Click $T.Solo;   Start-Sleep -Seconds $StepPause
  Step "Scroll down the guide (chevrons)"
  Invoke-Tool 'computer' @{ action = 'scroll'; tabId = $script:tabId; coordinate = $T.ScrollAt; scroll_direction = 'down'; scroll_amount = 5 } | Out-Null
  Start-Sleep -Seconds $StepPause
  Step "Back home via the breadcrumb -- closes the loop";      Click $T.Home;   Start-Sleep -Seconds $StepPause

  Write-Host ""
  Write-Host "Tour complete. Stop your recording." -ForegroundColor Green
  $exitCode = 0
}
catch {
  Write-Host ""
  Write-Host "TOUR FAILED: $($_.Exception.Message)" -ForegroundColor Red
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
