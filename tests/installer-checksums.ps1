# Reads release checksums through scripts/get.ps1's own functions and installs nothing. Offline it
# parses a SHA256SUMS payload shaped like a real release's, served as bytes the way GitHub serves
# application/octet-stream, and proves the script's body keeps its strict mode, preferences and
# variables out of the session that runs `irm | iex`. With -Tag it also reads that real release's
# SHA256SUMS from GitHub. It runs the same in Windows PowerShell 5.1 and PowerShell 7.
param([string]$Tag)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Set-StrictMode -Version Latest

$installer = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../scripts/get.ps1'))
$installerText = [IO.File]::ReadAllText($installer)
# `irm | iex` runs the text in the caller's scope. A malformed version stops the script before
# any request; the session it ran in keeps its own strict mode, preferences and variables.
& {
    Set-StrictMode -Off
    $ErrorActionPreference = 'Continue'
    $ProgressPreference = 'Continue'
    $pinned = $env:GHOSTLIGHT_VERSION
    $env:GHOSTLIGHT_VERSION = 'latest'
    try {
        $refused = $null
        try { $installerText | Invoke-Expression } catch { $refused = $_.Exception.Message }
        if ($refused -notlike 'GHOSTLIGHT_VERSION must look like*') { throw "The piped script stopped with: $refused" }
        if ($ErrorActionPreference -ne 'Continue' -or $ProgressPreference -ne 'Continue') {
            throw 'The piped script changed the session preferences.'
        }
        if (Test-Path variable:requested) { throw 'The piped script left its variables in the session.' }
        if (Test-Path function:Get-BoundChecksum) { throw 'The piped script left its functions in the session.' }
        $strict = $true
        try { $null = $undefinedBySession; $strict = $false } catch { }
        if ($strict) { throw 'The piped script left strict mode on in the session.' }
        # The documented script block form still binds -Restore.
        $refused = $null
        try { & ([scriptblock]::Create($installerText)) -Restore } catch { $refused = $_.Exception.Message }
        if ($refused -notlike 'GHOSTLIGHT_VERSION must look like*') { throw "The -Restore form stopped with: $refused" }
    } finally {
        $env:GHOSTLIGHT_VERSION = $pinned
    }
}

$tokens = $null
$errors = $null
$tree = [System.Management.Automation.Language.Parser]::ParseFile($installer, [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw "scripts/get.ps1 does not parse: $($errors[0].Message)" }
foreach ($name in @('Read-ReleaseText', 'Get-BoundChecksum')) {
    $definition = $tree.Find({
        param($node)
        $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name
    }, $true)
    if (-not $definition) { throw "scripts/get.ps1 defines no $name." }
    . ([scriptblock]::Create($definition.Extent.Text))
}

$windowsAssets = @('ghostlight', 'ghostlight-service', 'ghostlight-desktop') | ForEach-Object { "$_-x86_64-pc-windows-msvc.exe" }
# A real release lists every asset it carries, not only the three this script installs.
$otherAssets = @(
    'ghostlight-x86_64-unknown-linux-gnu', 'ghostlight-service-x86_64-unknown-linux-gnu',
    'ghostlight-desktop-x86_64-unknown-linux-gnu', 'ghostlight-v2.0.0.mcpb', 'ghostlight-2.0.0.tgz',
    'ghostlight-extension-v2.0.0.zip', 'release-candidate.json'
)
$expected = @{}
$index = 0
foreach ($asset in $windowsAssets + $otherAssets) {
    $index += 1
    $expected[$asset] = ('{0:x2}' -f $index) * 32
}
$area = Join-Path ([IO.Path]::GetTempPath()) "ghostlight-checksums-$([Guid]::NewGuid().ToString('N'))"
[IO.Directory]::CreateDirectory($area) | Out-Null
try {
    # sha256sum's own layout: the hash, two spaces, the name, one LF per line. GitHub serves the file
    # as application/octet-stream, so it arrives as bytes; the CRLF copy is what an editor may save.
    $global:checksumPayload = $null
    function Invoke-WebRequest {
        param([string]$Uri, [string]$OutFile, [switch]$UseBasicParsing)
        if (-not $OutFile) { return [pscustomobject]@{ Content = $global:checksumPayload } }
        [IO.File]::WriteAllBytes($OutFile, $global:checksumPayload)
    }
    foreach ($ending in @("`n", "`r`n")) {
        $text = (($expected.Keys | Sort-Object | ForEach-Object { "$($expected[$_])  $_" }) -join $ending) + $ending
        $global:checksumPayload = [Text.Encoding]::ASCII.GetBytes($text)
        $lines = Read-ReleaseText -Uri 'https://github.com/sylin-org/ghostlight/releases/download/v2.0.0/SHA256SUMS' -Path (Join-Path $area 'SHA256SUMS')
        foreach ($asset in $windowsAssets) {
            $bound = Get-BoundChecksum -Lines $lines -AssetName $asset
            if ($bound -ne $expected[$asset]) { throw "Wrong checksum for $asset`: $bound" }
        }
        foreach ($case in @(
            @{ Lines = @($lines | Where-Object { $_ -notlike '*  ghostlight-x86_64-pc-windows-msvc.exe' }); Said = 'absent' },
            @{ Lines = @($lines) + @($lines | Where-Object { $_ -like '*  ghostlight-x86_64-pc-windows-msvc.exe' }); Said = 'duplicated' }
        )) {
            $refused = $null
            try { [void](Get-BoundChecksum -Lines $case.Lines -AssetName 'ghostlight-x86_64-pc-windows-msvc.exe') } catch { $refused = $_.Exception.Message }
            if ($refused -ne 'SHA256SUMS does not bind exactly one ghostlight-x86_64-pc-windows-msvc.exe.') {
                throw "A $($case.Said) entry was not refused: $refused"
            }
        }
    }
    Remove-Item -LiteralPath function:Invoke-WebRequest

    if ($Tag) {
        if ($Tag -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$') { throw "-Tag must look like v1.3.14: $Tag" }
        $lines = Read-ReleaseText -Uri "https://github.com/sylin-org/ghostlight/releases/download/$Tag/SHA256SUMS" -Path (Join-Path $area 'SHA256SUMS')
        $bound = Get-BoundChecksum -Lines $lines -AssetName 'ghostlight-x86_64-pc-windows-msvc.exe'
        if ($bound -notmatch '^[0-9a-f]{64}$') { throw "The $Tag SHA256SUMS bound no checksum: $bound" }
        Write-Output "PASS installer checksums against the real $Tag SHA256SUMS in PowerShell $($PSVersionTable.PSVersion)"
    }
    Write-Output "PASS installer checksums: a byte payload shaped like a release's (LF and CRLF), absent and duplicate entries refused, the piped script leaves the session as it was, in PowerShell $($PSVersionTable.PSVersion)"
} finally {
    Remove-Item -LiteralPath $area -Recurse -Force
}
