# Real Linux ETXTBSY regression for the deployment controller's exact-image retry.
# Only test-owned executables and processes below the ordinary ignored test root are touched.
#Requires -Version 7
$ErrorActionPreference = "Stop"
if (-not $IsLinux) { throw "This executable-lock fixture requires Linux" }
$repository = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
. (Join-Path $repository "scripts/dev-loop.ps1") -Action Plan
$fixture = Join-Path $repository ".tmp/linux-startup-recovery/copy-retry-$([guid]::NewGuid())"
$destination = Join-Path $fixture "selected/sleep"
$neighbor = Join-Path $fixture "neighbor/sleep"
$replacement = Join-Path $fixture "replacement"
$owned = @()
try {
    foreach ($path in @($destination, $neighbor)) {
        [System.IO.Directory]::CreateDirectory((Split-Path -Parent $path)) | Out-Null
        [System.IO.File]::WriteAllBytes($path, [System.IO.File]::ReadAllBytes("/bin/sleep"))
        & chmod u+x $path
        if ($LASTEXITCODE -ne 0) { throw "Could not prepare the test executable" }
        $owned += Start-Process -FilePath $path -ArgumentList "30" -PassThru
    }
    foreach ($process in $owned) {
        if ($process.HasExited) { throw "The executable fixture exited before the test" }
    }
    [System.IO.File]::WriteAllText($replacement, "replacement bytes")
    $busy = $false
    try { Copy-Item -LiteralPath $replacement -Destination $destination -Force }
    catch { $busy = $true }
    if (-not $busy) { throw "The negative control did not observe an executable lock" }
    Copy-WithRetry -Source $replacement -Destination $destination
    $owned[0].Refresh()
    $owned[1].Refresh()
    if (-not $owned[0].HasExited) { throw "The selected respawned image was not stopped" }
    if ($owned[1].HasExited) { throw "Retry stopped an unrelated image" }
    if ([System.IO.File]::ReadAllText($destination) -ne "replacement bytes") {
        throw "Retry did not replace the selected destination"
    }
    Write-Output "copy retry passed: real executable lock; selected image stopped; neighbor retained"
}
finally {
    foreach ($process in $owned) {
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -ErrorAction SilentlyContinue }
    }
    Remove-Item -LiteralPath $fixture -Recurse -Force -ErrorAction SilentlyContinue
}
