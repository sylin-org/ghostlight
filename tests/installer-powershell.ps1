#Requires -Version 7
# Offline exercise of the shipped downloader and the real package installation seam: the edge, the
# service and the shell are verified and installed by the downloaded service, and a corrupt image
# installs nothing. The fixture sets GHOSTLIGHT_NO_REGISTER so the final `ghostlight install` never
# writes the machine's Start-menu shortcut or Uninstall key; the console output of that step is an
# owed live check.
param([string]$BinaryDirectory = $env:GHOSTLIGHT_BIN_DIR)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $BinaryDirectory) { $BinaryDirectory = Join-Path $repo '.target-ghostlight-1.0/debug' }
$BinaryDirectory = [IO.Path]::GetFullPath($BinaryDirectory)
$area = Join-Path $repo ".tmp/installer-powershell-$([Guid]::NewGuid().ToString('N'))"
[IO.Directory]::CreateDirectory($area) | Out-Null
$scenario = Join-Path $area 'scenario.ps1'
$scenarioSource = @'
param([string]$Repository, [string]$Binaries, [string]$UserDirectory)
$ErrorActionPreference = 'Stop'
$env:USERPROFILE = $UserDirectory
$env:HOME = $UserDirectory
$env:APPDATA = Join-Path $UserDirectory 'roaming'
$env:LOCALAPPDATA = Join-Path $UserDirectory 'local'
$env:CODEX_HOME = Join-Path $UserDirectory 'codex'
$env:GHOSTLIGHT_NATIVE_HOST_DIR = Join-Path $UserDirectory 'native-host'
$env:GHOSTLIGHT_NO_REGISTER = '1'
Remove-Item Env:GHOSTLIGHT_TEST_HOME -ErrorAction SilentlyContinue
$global:fixtureNames = @('ghostlight', 'ghostlight-service', 'ghostlight-desktop')
$global:fixtureBinaries = $Binaries
# SHA256SUMS as a release carries it: sha256sum's layout with one LF per line, listing the other
# system's images too, and served as application/octet-stream, so a response's Content is bytes.
$global:fixtureChecksums = [Text.Encoding]::ASCII.GetBytes((@($global:fixtureNames | ForEach-Object {
    $hash = (Get-FileHash -LiteralPath (Join-Path $global:fixtureBinaries "$_.exe") -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $_-x86_64-pc-windows-msvc.exe"
    "$('0' * 64)  $_-x86_64-unknown-linux-gnu"
}) -join "`n") + "`n")
$global:fixtureCorrupt = $false
$global:fixtureDownloads = [Collections.Generic.List[string]]::new()
class FixtureHttpError : System.Exception { [object]$Response }
$global:fixtureTags = @('v1.3.12', 'v2.0.0-rc.1')
$global:fixtureRequests = [Collections.Generic.List[string]]::new()
function New-FixtureRelease {
    param([string]$Tag)
    [pscustomobject]@{ draft = $false; prerelease = $Tag.Contains('-'); tag_name = $Tag; assets = @(
        @('SHA256SUMS') + @($global:fixtureNames | ForEach-Object { "$_-x86_64-pc-windows-msvc.exe" }) | ForEach-Object {
            [pscustomobject]@{ name = $_; browser_download_url = "https://github.com/sylin-org/ghostlight/releases/download/$Tag/$_" }
        }
    ) }
}
# The releases API: latest is the stable v1.3.12; a tag it has answers with that release, prerelease
# or not, any other tag answers 404, and v8.8.8 answers 403 (a rate limit).
function Invoke-RestMethod {
    param([string]$Uri)
    $global:fixtureRequests.Add($Uri)
    if ($Uri.EndsWith('/releases/latest')) { return New-FixtureRelease 'v1.3.12' }
    $tag = $Uri.Split('/')[-1]
    if ($Uri -like '*/releases/tags/*' -and $global:fixtureTags -contains $tag) { return New-FixtureRelease $tag }
    $failure = [FixtureHttpError]::new()
    $failure.Response = [pscustomobject]@{ StatusCode = $(if ($tag -eq 'v8.8.8') { 403 } else { 404 }) }
    throw $failure
}
function Invoke-WebRequest {
    param([string]$Uri, [string]$OutFile, [switch]$UseBasicParsing)
    if ($Uri.EndsWith('/SHA256SUMS')) {
        if (-not $OutFile) { return [pscustomobject]@{ Content = $global:fixtureChecksums } }
        [IO.File]::WriteAllBytes($OutFile, $global:fixtureChecksums)
        return
    }
    $name = ($Uri.Split('/')[-1] -replace '-x86_64-pc-windows-msvc', '')
    $global:fixtureDownloads.Add($OutFile)
    if ($global:fixtureCorrupt -and $name -eq 'ghostlight-service.exe') {
        [IO.File]::WriteAllText($OutFile, 'invalid final download')
    } else { Copy-Item -LiteralPath (Join-Path $global:fixtureBinaries $name) -Destination $OutFile }
}
function Get-Command {
    param([string]$Name, [string]$ErrorAction)
    if ($Name -eq 'gh') { return $null }
    Microsoft.PowerShell.Core\Get-Command -Name $Name
}
$installer = Join-Path $Repository 'scripts/get.ps1'
& $installer
$destination = Join-Path $UserDirectory '.ghostlight/bin'
foreach ($name in $global:fixtureNames) {
    if ((Get-FileHash (Join-Path $destination "$name.exe")).Hash -ne (Get-FileHash (Join-Path $Binaries "$name.exe")).Hash) {
        throw "Wrong installed bytes: $name"
    }
}
$before = (Get-FileHash (Join-Path $destination 'ghostlight.exe')).Hash
# A requested version is found by its own tag, prerelease included, and never asks for the latest
# release. A version that does not exist is refused in one sentence and changes nothing.
$global:fixtureRequests.Clear()
$env:GHOSTLIGHT_VERSION = '9.9.9'
$absent = $null
try { & $installer } catch { $absent = $_.Exception.Message }
if ($absent -ne 'Ghostlight release v9.9.9 does not exist.') { throw "Absent requested version: $absent" }
if ($global:fixtureRequests.Count -ne 1 -or $global:fixtureRequests[0] -ne 'https://api.github.com/repos/sylin-org/ghostlight/releases/tags/v9.9.9') {
    throw "Absent requested version asked for: $($global:fixtureRequests -join ', ')"
}
# Any other answer is a failed lookup and must not claim the release is absent.
$env:GHOSTLIGHT_VERSION = '8.8.8'
$limited = $null
try { & $installer } catch { $limited = $_.Exception.Message }
if ($null -eq $limited -or $limited -match 'does not exist') { throw "Rate-limited requested version: $limited" }
$env:GHOSTLIGHT_VERSION = 'latest'
$malformed = $null
try { & $installer } catch { $malformed = $_.Exception.Message }
if ($malformed -notmatch '^GHOSTLIGHT_VERSION must look like') { throw "Malformed requested version: $malformed" }
$global:fixtureRequests.Clear()
$env:GHOSTLIGHT_VERSION = '2.0.0-rc.1'
& $installer
Remove-Item Env:GHOSTLIGHT_VERSION
if ($global:fixtureRequests.Count -ne 1 -or $global:fixtureRequests[0] -ne 'https://api.github.com/repos/sylin-org/ghostlight/releases/tags/v2.0.0-rc.1') {
    throw "Pinned prerelease asked for: $($global:fixtureRequests -join ', ')"
}
$global:fixtureCorrupt = $true
$rejected = $false
try { & $installer } catch {
    if ($_.Exception.Message -notmatch 'Checksum verification failed') { throw }
    $rejected = $true
}
if (-not $rejected -or (Get-FileHash (Join-Path $destination 'ghostlight.exe')).Hash -ne $before) {
    throw 'A rejected download changed the installation'
}
foreach ($download in $global:fixtureDownloads) {
    if (Test-Path -LiteralPath ([IO.Path]::GetDirectoryName($download))) { throw 'Temporary download was retained' }
}
$expectedFiles = @($global:fixtureNames | ForEach-Object { "$_.exe" }) + 'install.json' | Sort-Object
$installedFiles = @(Get-ChildItem -LiteralPath $destination -File | ForEach-Object { $_.Name } | Sort-Object)
if (($installedFiles -join '|') -ne ($expectedFiles -join '|')) {
    throw "Unexpected installed files: $($installedFiles -join ', ')"
}
Write-Output 'PASS PowerShell installer: requested version by tag (prerelease too, absent refused), fixed paths, the three verified images, waited native install, space-containing profile, corrupt download preservation, temporary cleanup'
'@
[IO.File]::WriteAllText($scenario, $scenarioSource)
try {
    & pwsh -NoProfile -File $scenario -Repository $repo -Binaries $BinaryDirectory -UserDirectory (Join-Path $area 'user with spaces')
    if ($LASTEXITCODE -ne 0) { throw "PowerShell installer fixture failed: $LASTEXITCODE" }
} finally {
    # The install starts the fixture service and presence launches the shell, which can start
    # again after a first stop. Stop exactly the images of this fixture home, by path, then remove
    # the home, and repeat both until the removal succeeds or the time runs out.
    $installedBin = Join-Path $area 'user with spaces/.ghostlight/bin'
    $expectedRoot = $repo.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar + '.tmp' + [IO.Path]::DirectorySeparatorChar
    if (-not [IO.Path]::GetFullPath($area).StartsWith($expectedRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unsafe fixture cleanup: $area"
    }
    $stopFailures = @()
    $cleanupDeadline = [DateTime]::UtcNow.AddSeconds(15)
    while ($true) {
        foreach ($name in @('ghostlight-desktop', 'ghostlight-service')) {
            $image = Join-Path $installedBin "$name.exe"
            foreach ($process in @(Get-Process -Name $name -ErrorAction SilentlyContinue | Where-Object { $_.Path -ieq $image })) {
                try {
                    Stop-Process -InputObject $process -Force
                    if (-not $process.WaitForExit(5000)) { $stopFailures += "Fixture process $($process.Id) did not exit." }
                } catch { $stopFailures += $_.Exception.Message }
            }
        }
        try {
            Remove-Item -LiteralPath $area -Recurse -Force -ErrorAction Stop
            break
        } catch {
            if ([DateTime]::UtcNow -ge $cleanupDeadline) { throw }
            Start-Sleep -Milliseconds 250
        }
    }
    if ($stopFailures.Count) { throw ($stopFailures -join '; ') }
}
