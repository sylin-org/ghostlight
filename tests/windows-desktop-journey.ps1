# Exercise the real Windows workbench lifetime, including Open racing native startup.
# All window operations address only the exact no-argument process spawned by this journey.
param(
    [string] $BinDir = $env:GHOSTLIGHT_BIN_DIR,
    [ValidateRange(1, 10)] [int] $StartupRounds = 3,
    [ValidateRange(2, 24)] [int] $OpenBurst = 8
)

$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'This journey requires an interactive Windows desktop.' }
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $BinDir) { $BinDir = Join-Path $repository '.target-ghostlight-1.0/debug' }
$executable = (Resolve-Path -LiteralPath (Join-Path $BinDir 'ghostlight.exe')).Path
$binaryDirectory = Split-Path -Parent $executable
$stamp = [Guid]::NewGuid().ToString('N')
$evidence = Join-Path $repository ".tmp/native-desktop-$stamp"
$null = New-Item -ItemType Directory -Path $evidence
$env:GHOSTLIGHT_AUDIT_FILE = Join-Path $evidence 'audit.jsonl'
$env:GHOSTLIGHT_POLICY_FILE = Join-Path $evidence 'policy.json'
@{
    schema=3; name='Native desktop fixture'; version='1'
    grants=@(@{ id='ordinary'; hosts=@{ allow=@('*') }; allowed=@('read', 'action', 'write', 'execute') })
    config=@(@{ key='browser.startup'; value='manual'; level='mandatory' })
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $env:GHOSTLIGHT_POLICY_FILE -Encoding utf8
$env:GHOSTLIGHT_NATIVE_HOST_DIR = Join-Path $evidence 'native-host'
$env:GHOSTLIGHT_DIAGNOSTICS_DIR = Join-Path $evidence 'diagnostics'
$binaryHash = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash
Write-Output "Native desktop binary: $executable SHA256=$binaryHash"
Write-Output "Native desktop evidence: $evidence"

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class GhostlightNativeJourney {
    public class Window {
        public long Handle;
        public bool Visible;
        public bool Minimized;
        public bool Responsive;
    }
    private delegate bool EnumProc(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr window, StringBuilder name, int limit);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] private static extern bool IsIconic(IntPtr window);
    [DllImport("user32.dll")] private static extern IntPtr SendMessageTimeout(IntPtr window, uint message, UIntPtr w, IntPtr l, uint flags, uint timeout, out UIntPtr result);
    [DllImport("user32.dll")] private static extern bool ShowWindowAsync(IntPtr window, int command);
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr window, uint message, UIntPtr w, IntPtr l);
    public static Window[] Inspect(int owner) {
        var result = new List<Window>();
        EnumWindows((window, data) => {
            uint process;
            GetWindowThreadProcessId(window, out process);
            if (process != owner) return true;
            var name = new StringBuilder(256);
            GetClassName(window, name, name.Capacity);
            if (name.ToString() != "Tauri Window") return true;
            UIntPtr ignored;
            result.Add(new Window {
                Handle=window.ToInt64(), Visible=IsWindowVisible(window), Minimized=IsIconic(window),
                Responsive=SendMessageTimeout(window, 0, UIntPtr.Zero, IntPtr.Zero, 2, 250, out ignored) != IntPtr.Zero
            });
            return true;
        }, IntPtr.Zero);
        return result.ToArray();
    }
    public static void Control(int owner, long handle, bool close) {
        var window = new IntPtr(handle);
        uint process;
        GetWindowThreadProcessId(window, out process);
        if (process != owner) throw new InvalidOperationException("Window ownership changed.");
        if (close) {
            if (!PostMessage(window, 0x0010, UIntPtr.Zero, IntPtr.Zero)) throw new InvalidOperationException("WM_CLOSE failed.");
        } else if (!ShowWindowAsync(window, 6)) throw new InvalidOperationException("Minimize failed.");
    }
}
'@

$children = [Collections.Generic.List[Diagnostics.Process]]::new()
$observations = [Collections.Generic.List[object]]::new()
$runtimeFiles = [Collections.Generic.List[string]]::new()
$script:launch = 0
$failure = $null

function Start-OwnedProcess([string[]] $Arguments = @()) {
    $script:launch++
    $options = @{
        FilePath = $executable; PassThru = $true; WindowStyle = 'Hidden'
        RedirectStandardOutput = (Join-Path $evidence "$script:launch.stdout.log")
        RedirectStandardError = (Join-Path $evidence "$script:launch.stderr.log")
    }
    if ($Arguments.Count) { $options.ArgumentList = $Arguments }
    $child = Start-Process @options
    $children.Add($child)
    return $child
}

function Get-OwnedWindows([Diagnostics.Process] $Authority) {
    $Authority.Refresh()
    if ($Authority.HasExited) { throw "Authority $($Authority.Id) exited unexpectedly with $($Authority.ExitCode)." }
    if ($Authority.MainModule.FileName -ine $executable) { throw 'Authority executable ownership changed.' }
    $windows = @([GhostlightNativeJourney]::Inspect($Authority.Id))
    if ($windows.Count -gt 1) {
        $details = $windows | ConvertTo-Json -Compress
        throw "Duplicate native workbenches in owned PID $($Authority.Id): $details"
    }
    return $windows
}

function Wait-WindowState([Diagnostics.Process] $Authority, [string] $State) {
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $windows = @(Get-OwnedWindows $Authority)
        $ready = switch ($State) {
            'closed' { $windows.Count -eq 0 }
            'minimized' { $windows.Count -eq 1 -and $windows[0].Responsive -and $windows[0].Minimized }
            'restored' { $windows.Count -eq 1 -and $windows[0].Responsive -and $windows[0].Visible -and -not $windows[0].Minimized }
            default { throw "Unknown native state $State" }
        }
        if ($ready) { return $windows }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for $State workbench in PID $($Authority.Id)."
}

function Wait-OpenBurst([Diagnostics.Process] $Authority, [object[]] $Openers) {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        $null = @(Get-OwnedWindows $Authority)
        $pending = 0
        foreach ($opener in $Openers) {
            $opener.Refresh()
            if (-not $opener.HasExited) { $pending++; continue }
            if ($opener.ExitCode -ne 0) { throw "Open process $($opener.Id) failed: $($opener.ExitCode)." }
        }
        if ($pending -eq 0) { return }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Concurrent Open requests did not settle.'
}

function Record-State([int] $Round, [string] $State, [Diagnostics.Process] $Authority) {
    $windows = @(Get-OwnedWindows $Authority)
    $observations.Add(@{ round=$Round; state=$State; pid=$Authority.Id; windows=$windows })
    Write-Output "PASS round=$Round $State count=$($windows.Count)"
}

function Assert-IsolatedWebView([int] $Round, [Diagnostics.Process] $Authority) {
    # Query only children of the exact owned authority. Never collect unrelated browser profiles.
    $null = @(Get-OwnedWindows $Authority)
    $webViews = @(Get-CimInstance -ClassName Win32_Process -Filter "ParentProcessId = $($Authority.Id) AND Name = 'msedgewebview2.exe'")
    $profiles = @()
    foreach ($webView in $webViews) {
        if ($webView.CommandLine -match '(?:^|\s)--user-data-dir=(?:"([^"]+)"|(\S+))') {
            $profile = if ($Matches[1]) { $Matches[1] } else { $Matches[2] }
            $profiles += [IO.Path]::GetFullPath($profile)
        }
    }
    # WebView2 keeps Chromium's browser profile in EBWebView beneath the selected UDF.
    $expected = [IO.Path]::GetFullPath((Join-Path $env:WEBVIEW2_USER_DATA_FOLDER 'EBWebView'))
    if (-not $profiles.Count -or @($profiles | Where-Object { $_ -ine $expected }).Count) {
        throw 'Owned WebView2 did not use its isolated evidence directory.'
    }
    if (-not (Test-Path -LiteralPath $expected -PathType Container)) {
        throw 'Owned WebView2 did not create its isolated data directory.'
    }
    $observations.Add(@{ round=$Round; state='isolated-webview-profile'; pid=$Authority.Id; directory=$expected })
    Write-Output "PASS round=$Round isolated-webview-profile"
}

function Stop-OwnedProcess([Diagnostics.Process] $Child) {
    $Child.Refresh()
    if (-not $Child.HasExited) {
        if ($Child.MainModule.FileName -ine $executable) { throw 'Cleanup executable ownership changed.' }
        Stop-Process -InputObject $Child -Force
        if (-not $Child.WaitForExit(5000)) { throw "Owned child $($Child.Id) did not exit." }
    }
}

try {
    for ($round = 1; $round -le $StartupRounds; $round++) {
        # The runtime override stays beside the exact executable under test (ADR-0150).
        $runtime = Join-Path $binaryDirectory ".ghostlight-native-$stamp-$round.json"
        $runtimeFiles.Add($runtime)
        $runtimeFiles.Add([IO.Path]::ChangeExtension($runtime, '.lock'))
        $env:GHOSTLIGHT_RUNTIME_FILE = $runtime
        $env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $evidence "webview2-$round"
        $authority = Start-OwnedProcess
        $deadline = [DateTime]::UtcNow.AddSeconds(15)
        while (-not (Test-Path -LiteralPath $runtime)) {
            $authority.Refresh()
            if ($authority.HasExited) { throw 'The ordinary desktop launch exited before publishing discovery.' }
            if ([DateTime]::UtcNow -gt $deadline) { throw 'Desktop runtime discovery was not published.' }
            Start-Sleep -Milliseconds 2
        }
        # Publication precedes Tauri Ready. Start the real authenticated Open calls immediately.
        $openers = @(for ($index = 0; $index -lt $OpenBurst; $index++) { Start-OwnedProcess @('open') })
        Wait-OpenBurst $authority $openers
        $window = @(Wait-WindowState $authority 'restored')[0]
        for ($sample = 0; $sample -lt 10; $sample++) {
            $null = @(Get-OwnedWindows $authority)
            Start-Sleep -Milliseconds 50
        }
        Record-State $round 'startup-open-burst' $authority
        Assert-IsolatedWebView $round $authority

        $minimizedHandle = $window.Handle
        [GhostlightNativeJourney]::Control($authority.Id, $window.Handle, $false)
        $null = @(Wait-WindowState $authority 'minimized')
        Record-State $round 'native-minimize' $authority
        Wait-OpenBurst $authority @((Start-OwnedProcess @('open')))
        $window = @(Wait-WindowState $authority 'restored')[0]
        if ($window.Handle -ne $minimizedHandle) { throw 'Open replaced the minimized native window instead of restoring it.' }
        Record-State $round 'restore-existing' $authority

        [GhostlightNativeJourney]::Control($authority.Id, $window.Handle, $true)
        $null = @(Wait-WindowState $authority 'closed')
        Record-State $round 'native-close-authority-alive' $authority
        $openers = @(for ($index = 0; $index -lt $OpenBurst; $index++) { Start-OwnedProcess @('open') })
        Wait-OpenBurst $authority $openers
        $null = @(Wait-WindowState $authority 'restored')
        Record-State $round 'reopen-burst-one-replacement' $authority
        Stop-OwnedProcess $authority
    }
    Write-Output "Windows native desktop journey passed: $($observations.Count) lifecycle checks."
} catch {
    $failure = $_.Exception.Message
    throw
} finally {
    $cleanupFailures = @()
    # Stop Open callers first so none can demand-start a replacement during cleanup.
    for ($index = $children.Count - 1; $index -ge 0; $index--) {
        try { Stop-OwnedProcess $children[$index] }
        catch { $cleanupFailures += $_.Exception.Message }
    }
    if (-not $cleanupFailures.Count) {
        foreach ($runtime in $runtimeFiles) {
            try { if (Test-Path -LiteralPath $runtime) { Remove-Item -LiteralPath $runtime -Force } }
            catch { $cleanupFailures += $_.Exception.Message }
        }
    }
    $failureCode = if ($failure) { 'native_lifecycle' } elseif ($cleanupFailures.Count) { 'cleanup' } else { $null }
    @{ executable=$executable; sha256=$binaryHash; passed=($null -eq $failureCode); failure_code=$failureCode; failure=$failure; cleanup_failures=$cleanupFailures; rounds=$StartupRounds; open_burst=$OpenBurst; observations=$observations } |
        ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence 'results.json') -Encoding utf8
    if ($cleanupFailures.Count -and -not $failure) { throw ($cleanupFailures -join '; ') }
}
