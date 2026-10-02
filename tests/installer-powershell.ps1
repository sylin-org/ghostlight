#Requires -Version 7
# Offline exercise of the shipped downloader and real native package installation seam.
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
Remove-Item Env:GHOSTLIGHT_RUNTIME_FILE -ErrorAction SilentlyContinue
$global:fixtureNames = @('ghostlight', 'ghostlight-mcp-connector', 'ghostlight-browser-connector')
$global:fixtureBinaries = $Binaries
$global:fixtureChecksums = @($global:fixtureNames | ForEach-Object {
    $hash = (Get-FileHash -LiteralPath (Join-Path $global:fixtureBinaries "$_.exe") -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $_-x86_64-pc-windows-msvc.exe"
}) -join "`n"
$global:fixtureCorrupt = $false
$global:fixtureDownloads = [Collections.Generic.List[string]]::new()
function Invoke-RestMethod {
    param([string]$Uri)
    return [pscustomobject]@{ draft = $false; prerelease = $false; tag_name = 'v1.3.12'; assets = @(
        @('SHA256SUMS') + @($global:fixtureNames | ForEach-Object { "$_-x86_64-pc-windows-msvc.exe" }) | ForEach-Object {
            [pscustomobject]@{ name = $_; browser_download_url = "https://github.com/sylin-org/ghostlight/releases/download/v1.3.12/$_" }
        }
    ) }
}
function Invoke-WebRequest {
    param([string]$Uri, [string]$OutFile, [switch]$UseBasicParsing)
    if ($Uri.EndsWith('/SHA256SUMS')) { return [pscustomobject]@{ Content = $global:fixtureChecksums } }
    $name = ($Uri.Split('/')[-1] -replace '-x86_64-pc-windows-msvc', '')
    $global:fixtureDownloads.Add($OutFile)
    if ($global:fixtureCorrupt -and $name -eq 'ghostlight-browser-connector.exe') {
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
if ((Get-ChildItem -LiteralPath $destination).Count -ne 3) { throw 'Unexpected installed copies' }
Write-Output 'PASS PowerShell installer: fixed paths, complete verified payload, waited native install, space-containing profile, corrupt download preservation, temporary cleanup'
'@
[IO.File]::WriteAllText($scenario, $scenarioSource)
try {
    & pwsh -NoProfile -File $scenario -Repository $repo -Binaries $BinaryDirectory -UserDirectory (Join-Path $area 'user with spaces')
    if ($LASTEXITCODE -ne 0) { throw "PowerShell installer fixture failed: $LASTEXITCODE" }
} finally {
    $expectedRoot = $repo.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar + '.tmp' + [IO.Path]::DirectorySeparatorChar
    if (-not [IO.Path]::GetFullPath($area).StartsWith($expectedRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unsafe fixture cleanup: $area"
    }
    Remove-Item -LiteralPath $area -Recurse -Force
}
