#Requires -Version 7
<#
.SYNOPSIS
    One-time helper to mint a Chrome Web Store API refresh token via the OAuth2 loopback flow.

.DESCRIPTION
    Google retired the copy-paste "oob" flow, so this uses a loopback redirect: it starts a tiny
    local listener on 127.0.0.1:<Port>, opens the Google consent page in your browser, catches the
    redirect that carries the authorization code, exchanges it for tokens, and prints ONLY the
    refresh token. Store that as CWS_REFRESH_TOKEN in your out-of-repo values file (see
    local/RELEASE-CREDENTIALS.md) -- never commit it.

    It reads CWS_CLIENT_ID and CWS_CLIENT_SECRET from the environment, or prompts for them. It
    never writes any secret to disk.

    Prerequisite: a Google Cloud OAuth client of type "Desktop app" with the Chrome Web Store API
    enabled, and yourself added as a test user on the consent screen (see the guide).

.PARAMETER Port
    Loopback port for the redirect. Must be free. Default 8976. (The redirect_uri registered in the
    request is http://localhost:<Port>; Desktop-app clients accept any loopback port.)

.PARAMETER TimeoutSec
    How long to wait for the browser redirect before giving up. Default 180.

.EXAMPLE
    $env:CWS_CLIENT_ID='...'; $env:CWS_CLIENT_SECRET='...'
    pwsh -File scripts/get-cws-refresh-token.ps1
#>
[CmdletBinding()]
param(
    [int] $Port = 8976,
    [int] $TimeoutSec = 180,
    # Write CWS_REFRESH_TOKEN straight into the env file (default: $HOME/.ghostlight-release.env)
    # instead of printing it, so the token never lands on a terminal or in a transcript.
    [switch] $Store,
    [string] $EnvFile = (Join-Path $HOME '.ghostlight-release.env'),
    # Pre-select this Google account on the consent page (helpful when signed into several).
    [string] $LoginHint
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Auto-load the release env file if the client creds are not already in the environment (so this
# helper "just works" after set-credentials.ps1, with no manual sourcing step).
if ((-not $env:CWS_CLIENT_ID -or -not $env:CWS_CLIENT_SECRET) -and (Test-Path $EnvFile)) {
    Get-Content $EnvFile | ForEach-Object {
        if ($_ -match '^([A-Z0-9_]+)=(.*)$') { [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2]) }
    }
}

$clientId = if ($env:CWS_CLIENT_ID) { $env:CWS_CLIENT_ID } else { Read-Host 'CWS_CLIENT_ID' }
$clientSecret = if ($env:CWS_CLIENT_SECRET) { $env:CWS_CLIENT_SECRET } else { Read-Host 'CWS_CLIENT_SECRET' }
if (-not $clientId -or -not $clientSecret) { throw 'need both CWS_CLIENT_ID and CWS_CLIENT_SECRET' }

$redirect = "http://localhost:$Port"
$scope = 'https://www.googleapis.com/auth/chromewebstore'
$authUrl = 'https://accounts.google.com/o/oauth2/auth' +
    "?response_type=code&access_type=offline&prompt=consent" +
    "&redirect_uri=$([uri]::EscapeDataString($redirect))" +
    "&scope=$([uri]::EscapeDataString($scope))" +
    "&client_id=$([uri]::EscapeDataString($clientId))" +
    $(if ($LoginHint) { "&login_hint=$([uri]::EscapeDataString($LoginHint))" } else { '' })

# Extract a query-string value from a raw HTTP request line ("GET /?code=x&scope=y HTTP/1.1").
function Get-QueryValue([string] $RequestLine, [string] $Key) {
    if ($RequestLine -notmatch 'GET\s+(\S+)\s') { return $null }
    $q = $Matches[1]
    if ($q -notmatch '\?(.*)$') { return $null }
    foreach ($pair in ($Matches[1] -split '&')) {
        $kv = $pair -split '=', 2
        if ($kv[0] -eq $Key -and $kv.Count -eq 2) { return [uri]::UnescapeDataString($kv[1]) }
    }
    return $null
}

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $Port)
try { $listener.Start() }
catch { throw "could not listen on 127.0.0.1:$Port ($($_.Exception.Message)); pick another -Port" }

Write-Host "Opening the Google consent page in your browser..." -ForegroundColor Cyan
Write-Host "If it does not open, paste this URL manually:`n  $authUrl`n"
try { Start-Process $authUrl } catch { Write-Host '(could not auto-open a browser; paste the URL above)' }

Write-Host "Waiting up to ${TimeoutSec}s for the redirect on $redirect ..."
$deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSec)
$code = $null
$authError = $null
while ([DateTime]::UtcNow -lt $deadline -and -not $code -and -not $authError) {
    if (-not $listener.Pending()) { Start-Sleep -Milliseconds 200; continue }
    $client = $listener.AcceptTcpClient()
    try {
        $stream = $client.GetStream()
        $reader = [System.IO.StreamReader]::new($stream)
        $requestLine = $reader.ReadLine()
        $gotCode = Get-QueryValue $requestLine 'code'
        $gotErr = Get-QueryValue $requestLine 'error'

        $body = if ($gotCode) {
            '<h2>Ghostlight: authorization received.</h2><p>You can close this tab and return to the terminal.</p>'
        }
        elseif ($gotErr) { "<h2>Authorization failed: $gotErr</h2>" }
        else { '<h2>Waiting...</h2>' } # favicon or stray request
        $writer = [System.IO.StreamWriter]::new($stream)
        $writer.WriteLine('HTTP/1.1 200 OK')
        $writer.WriteLine('Content-Type: text/html; charset=utf-8')
        $writer.WriteLine("Content-Length: $([System.Text.Encoding]::UTF8.GetByteCount($body))")
        $writer.WriteLine('Connection: close')
        $writer.WriteLine('')
        $writer.Write($body)
        $writer.Flush()

        if ($gotCode) { $code = $gotCode }
        elseif ($gotErr) { $authError = $gotErr }
    }
    finally { $client.Close() }
}
$listener.Stop()

if ($authError) { throw "Google returned an error: $authError" }
if (-not $code) { throw "no authorization code received within ${TimeoutSec}s (did you consent in the browser?)" }
Write-Host '  [ok] authorization code received' -ForegroundColor Green

# Exchange the code for tokens.
$resp = Invoke-RestMethod -Method Post -Uri 'https://oauth2.googleapis.com/token' -Body @{
    client_id     = $clientId
    client_secret = $clientSecret
    code          = $code
    grant_type    = 'authorization_code'
    redirect_uri  = $redirect
}
if (-not $resp.refresh_token) {
    throw 'the token response carried no refresh_token. Re-run with a FRESH consent (prompt=consent forces it); a refresh token is only returned on first consent per client unless forced.'
}

if ($Store) {
    # Merge CWS_REFRESH_TOKEN into the env file in place, without ever printing the token.
    $rt = $resp.refresh_token
    $lines = if (Test-Path $EnvFile) { @(Get-Content $EnvFile) } else { @() }
    $out = [System.Collections.Generic.List[string]]::new()
    $seen = $false
    foreach ($line in $lines) {
        if ($line -match '^\s*CWS_REFRESH_TOKEN\s*=') { $out.Add("CWS_REFRESH_TOKEN=$rt"); $seen = $true }
        else { $out.Add($line) }
    }
    if (-not $seen) { $out.Add("CWS_REFRESH_TOKEN=$rt") }
    Set-Content -Path $EnvFile -Value $out -Encoding utf8
    if (-not $IsWindows) { chmod 600 $EnvFile }
    Write-Host ''
    Write-Host "SUCCESS. Stored CWS_REFRESH_TOKEN in $EnvFile (length $($rt.Length), value hidden)." -ForegroundColor Green
    Write-Host 'Verify all four CWS_* are set:  pwsh -File scripts/publish-extension.ps1 -DryRun'
}
else {
    Write-Host ''
    Write-Host 'SUCCESS. Store this as CWS_REFRESH_TOKEN in your out-of-repo values file (never commit it):' -ForegroundColor Green
    Write-Host ''
    Write-Host "  $($resp.refresh_token)"
    Write-Host ''
    Write-Host 'Store it:  pwsh -File local/set-credentials.ps1 CWS_REFRESH_TOKEN <token>'
    Write-Host 'Then verify:  pwsh -File scripts/publish-extension.ps1 -DryRun'
}
