#Requires -Version 7
# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$CredentialFile = (Join-Path $HOME ".ghostlight-release.env"),
    [ValidateRange(1024, 65535)]
    [int]$Port = 8976
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$values = @{}
if (Test-Path -LiteralPath $CredentialFile -PathType Leaf) {
    foreach ($line in Get-Content -LiteralPath $CredentialFile) {
        if ($line -match '^([A-Z0-9_]+)=(.*)$') {
            $values[$Matches[1]] = $Matches[2]
        }
    }
}
foreach ($name in @("CWS_CLIENT_ID", "CWS_CLIENT_SECRET")) {
    if (-not $values.ContainsKey($name) -or [string]::IsNullOrWhiteSpace($values[$name])) {
        throw "$name is missing from $CredentialFile"
    }
}

function ConvertTo-Base64Url {
    param([byte[]]$Bytes)
    return [Convert]::ToBase64String($Bytes).TrimEnd("=").Replace("+", "-").Replace("/", "_")
}

function Get-QueryValue {
    param([string]$Query, [string]$Name)
    foreach ($pair in $Query.TrimStart("?").Split("&")) {
        $parts = $pair.Split("=", 2)
        if ([uri]::UnescapeDataString($parts[0]) -eq $Name) {
            if ($parts.Count -eq 2) {
                return [uri]::UnescapeDataString($parts[1])
            }
            return ""
        }
    }
    return $null
}

$random = [System.Security.Cryptography.RandomNumberGenerator]::Create()
$stateBytes = [byte[]]::new(32)
$verifierBytes = [byte[]]::new(64)
$random.GetBytes($stateBytes)
$random.GetBytes($verifierBytes)
$random.Dispose()
$state = ConvertTo-Base64Url -Bytes $stateBytes
$verifier = ConvertTo-Base64Url -Bytes $verifierBytes
$sha = [System.Security.Cryptography.SHA256]::Create()
$challenge = ConvertTo-Base64Url -Bytes $sha.ComputeHash([Text.Encoding]::ASCII.GetBytes($verifier))
$sha.Dispose()

$redirectUri = "http://127.0.0.1:$Port/"
$parameters = [ordered]@{
    client_id = $values.CWS_CLIENT_ID
    redirect_uri = $redirectUri
    response_type = "code"
    scope = "https://www.googleapis.com/auth/chromewebstore"
    access_type = "offline"
    prompt = "consent"
    state = $state
    code_challenge = $challenge
    code_challenge_method = "S256"
}
$query = @($parameters.GetEnumerator() | ForEach-Object {
    "$([uri]::EscapeDataString($_.Key))=$([uri]::EscapeDataString($_.Value))"
}) -join "&"
$authorizationUri = "https://accounts.google.com/o/oauth2/v2/auth?$query"

$listener = [System.Net.HttpListener]::new()
$listener.Prefixes.Add($redirectUri)
try {
    $listener.Start()
    Write-Output "Opening Google consent in the system browser. The callback is accepted only on 127.0.0.1."
    Start-Process $authorizationUri
    $contextTask = $listener.GetContextAsync()
    if (-not $contextTask.Wait([TimeSpan]::FromMinutes(5))) {
        throw "Timed out waiting for Google OAuth consent"
    }
    $context = $contextTask.Result
    $receivedState = Get-QueryValue -Query $context.Request.Url.Query -Name "state"
    $code = Get-QueryValue -Query $context.Request.Url.Query -Name "code"
    $oauthError = Get-QueryValue -Query $context.Request.Url.Query -Name "error"

    $message = if ($oauthError) {
        "Ghostlight release access was not granted. You may close this tab."
    }
    else {
        "Ghostlight release access was received. You may close this tab."
    }
    $body = [Text.Encoding]::UTF8.GetBytes("<!doctype html><meta charset=utf-8><title>Ghostlight release access</title><p>$message</p>")
    $context.Response.StatusCode = 200
    $context.Response.ContentType = "text/html; charset=utf-8"
    $context.Response.ContentLength64 = $body.Length
    $context.Response.OutputStream.Write($body, 0, $body.Length)
    $context.Response.OutputStream.Close()

    if ($oauthError) {
        throw "Google OAuth returned: $oauthError"
    }
    if ($receivedState -ne $state -or [string]::IsNullOrWhiteSpace($code)) {
        throw "Google OAuth callback failed state or code validation"
    }

    $token = Invoke-RestMethod `
        -Method Post `
        -Uri "https://oauth2.googleapis.com/token" `
        -ContentType "application/x-www-form-urlencoded" `
        -Body @{
            client_id = $values.CWS_CLIENT_ID
            client_secret = $values.CWS_CLIENT_SECRET
            code = $code
            code_verifier = $verifier
            grant_type = "authorization_code"
            redirect_uri = $redirectUri
        }
    if ([string]::IsNullOrWhiteSpace($token.refresh_token)) {
        throw "Google returned no refresh token; revoke the old grant and retry with consent"
    }

    $existing = if (Test-Path -LiteralPath $CredentialFile) { @(Get-Content -LiteralPath $CredentialFile) } else { @() }
    $output = [System.Collections.Generic.List[string]]::new()
    $replaced = $false
    foreach ($line in $existing) {
        if ($line -match '^CWS_REFRESH_TOKEN=') {
            [void]$output.Add("CWS_REFRESH_TOKEN=$($token.refresh_token)")
            $replaced = $true
        }
        else {
            [void]$output.Add($line)
        }
    }
    if (-not $replaced) {
        [void]$output.Add("CWS_REFRESH_TOKEN=$($token.refresh_token)")
    }
    [System.IO.File]::WriteAllLines(
        [System.IO.Path]::GetFullPath($CredentialFile),
        $output,
        [System.Text.UTF8Encoding]::new($false)
    )
    if (-not $IsWindows) {
        & chmod 600 $CredentialFile
    }
    Write-Output "Stored a new CWS_REFRESH_TOKEN in $CredentialFile. The token value was not printed."
}
finally {
    if ($listener.IsListening) {
        $listener.Stop()
    }
    $listener.Close()
}
