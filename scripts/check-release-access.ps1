# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$CredentialFile = (Join-Path $HOME ".ghostlight-release.env"),
    [switch]$Online,
    [switch]$RequireReady
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$values = @{}
if (Test-Path -LiteralPath $CredentialFile -PathType Leaf) {
    foreach ($line in Get-Content -LiteralPath $CredentialFile) {
        if ($line -match '^([A-Z0-9_]+)=(.*)$') {
            $values[$Matches[1]] = $Matches[2]
        }
    }
}

function Test-Present {
    param([string]$Name)
    return $values.ContainsKey($Name) -and -not [string]::IsNullOrWhiteSpace($values[$Name])
}

function Write-AccessState {
    param([string]$Name, [string]$State)
    Write-Output "$Name=$State"
}

$problems = [System.Collections.Generic.List[string]]::new()
$chromeAutomation = @(
    "CWS_CLIENT_ID",
    "CWS_CLIENT_SECRET",
    "CWS_REFRESH_TOKEN",
    "CWS_ITEM_ID",
    "CWS_PUBLISHER_ID"
)
foreach ($name in $chromeAutomation) {
    $state = if (Test-Present -Name $name) { "optional-present" } else { "optional-not-configured" }
    Write-AccessState -Name $name -State $state
}
$chromeAutomationReady = @($chromeAutomation | Where-Object { -not (Test-Present -Name $_) }).Count -eq 0

$mcpKeyPresent = Test-Present -Name "MCP_DNS_PRIVATE_KEY"
Write-AccessState -Name "MCP_DNS_PRIVATE_KEY" -State $(if ($mcpKeyPresent) { "optional-present" } else { "optional-not-configured" })
if (-not $mcpKeyPresent) {
    Write-Output "MCP Registry automation is optional; manual or deferred publication remains available."
}

$publisher = Get-Command "mcp-publisher" -ErrorAction SilentlyContinue
if ($null -eq $publisher) {
    $localPublisher = Join-Path $repo "local/mcp-publisher.exe"
    if (Test-Path -LiteralPath $localPublisher -PathType Leaf) {
        $publisher = Get-Item -LiteralPath $localPublisher
    }
}
Write-AccessState -Name "MCP_PUBLISHER" -State $(if ($null -ne $publisher) { "optional-present" } else { "optional-not-configured" })
if ($null -eq $publisher) {
    Write-Output "MCP Registry publisher automation is optional."
}

$githubState = "not-checked"
$npmState = "not-checked"
$chromeState = if ($chromeAutomationReady) { "configured-not-checked" } else { "manual-dashboard" }
if ($Online) {
    & gh auth status *> $null
    $githubState = if ($LASTEXITCODE -eq 0) { "valid" } else { "invalid" }
    if ($githubState -ne "valid") {
        [void]$problems.Add("GitHub authentication is invalid")
    }

    & npm whoami *> $null
    $npmState = if ($LASTEXITCODE -eq 0) { "valid" } else { "invalid" }
    if ($npmState -ne "valid") {
        [void]$problems.Add("npm authentication is invalid")
    }

    $oauthNames = @("CWS_CLIENT_ID", "CWS_CLIENT_SECRET", "CWS_REFRESH_TOKEN")
    $oauthReady = @($oauthNames | Where-Object { -not (Test-Present -Name $_) }).Count -eq 0
    if ($oauthReady) {
        $tokenResponse = Invoke-WebRequest `
            -SkipHttpErrorCheck `
            -Method Post `
            -Uri "https://oauth2.googleapis.com/token" `
            -ContentType "application/x-www-form-urlencoded" `
            -Body @{
                client_id = $values.CWS_CLIENT_ID
                client_secret = $values.CWS_CLIENT_SECRET
                refresh_token = $values.CWS_REFRESH_TOKEN
                grant_type = "refresh_token"
            }
        if ($tokenResponse.StatusCode -eq 200) {
            $token = $tokenResponse.Content | ConvertFrom-Json
            $chromeState = "oauth-valid"
            if (Test-Present -Name "CWS_PUBLISHER_ID" -and Test-Present -Name "CWS_ITEM_ID") {
                $resource = "publishers/$($values.CWS_PUBLISHER_ID)/items/$($values.CWS_ITEM_ID)"
                $statusResponse = Invoke-WebRequest `
                    -SkipHttpErrorCheck `
                    -Method Get `
                    -Uri "https://chromewebstore.googleapis.com/v2/${resource}:fetchStatus" `
                    -Headers @{ Authorization = "Bearer $($token.access_token)" }
                if ($statusResponse.StatusCode -eq 200) {
                    $chromeState = "v2-item-valid"
                }
                else {
                    $chromeState = "v2-item-invalid-http-$($statusResponse.StatusCode)"
                }
            }
        }
        else {
            $body = $tokenResponse.Content | ConvertFrom-Json
            $errorName = if ($body.error) { $body.error } else { "http-$($tokenResponse.StatusCode)" }
            $chromeState = "oauth-invalid-$errorName"
        }
    }
    else {
        $chromeState = "oauth-not-attempted"
    }
}

Write-AccessState -Name "GITHUB_AUTH" -State $githubState
Write-AccessState -Name "NPM_AUTH" -State $npmState
Write-AccessState -Name "CHROME_WEB_STORE" -State $chromeState
Write-Output "Chrome API automation is optional; manual Developer Dashboard submission remains available."

if ($RequireReady -and $problems.Count -gt 0) {
    throw "Release access is not ready: $($problems -join '; ')"
}
