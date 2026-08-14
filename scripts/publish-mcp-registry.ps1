#Requires -Version 7
# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$CandidateDirectory,
    [string]$ServerFile = "server.json",
    [string]$CredentialFile = (Join-Path $HOME ".ghostlight-release.env"),
    [ValidateSet("Plan", "Publish")]
    [string]$Action = "Plan",
    [switch]$Execute
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ($Action -ne "Plan" -and -not $Execute) {
    throw "$Action changes the MCP Registry. Pass -Execute only after owner approval for that exact action."
}

$repo = Split-Path -Parent $PSScriptRoot
$CandidateDirectory = [System.IO.Path]::GetFullPath($CandidateDirectory)
$ServerFile = [System.IO.Path]::GetFullPath((Join-Path $repo $ServerFile))
$candidatePath = Join-Path $CandidateDirectory "release-candidate.json"
if (-not (Test-Path -LiteralPath $candidatePath -PathType Leaf)) {
    throw "Candidate manifest does not exist: $candidatePath"
}
if (-not (Test-Path -LiteralPath $ServerFile -PathType Leaf)) {
    throw "MCP server metadata does not exist: $ServerFile"
}
$candidate = Get-Content -LiteralPath $candidatePath -Raw | ConvertFrom-Json
& (Join-Path $PSScriptRoot "check-release-candidate.ps1") -CandidateDirectory $CandidateDirectory
$server = Get-Content -LiteralPath $ServerFile -Raw | ConvertFrom-Json
$publisher = Get-Command "mcp-publisher" -ErrorAction SilentlyContinue
if ($null -eq $publisher) {
    $localPublisher = Join-Path $repo "local/mcp-publisher.exe"
    if (Test-Path -LiteralPath $localPublisher -PathType Leaf) {
        $publisher = Get-Item -LiteralPath $localPublisher
    }
}
if ($null -eq $publisher) {
    throw "mcp-publisher is not installed and no machine-local publisher was recovered"
}

$problems = [System.Collections.Generic.List[string]]::new()
if ($server.name -ne "org.sylin/ghostlight") {
    [void]$problems.Add("server name is not org.sylin/ghostlight")
}
if ($server.version -ne $candidate.version) {
    [void]$problems.Add("server version $($server.version) differs from candidate $($candidate.version)")
}
$packages = @($server.packages)
if ($packages.Count -ne 1 -or $packages[0].registryType -ne "npm") {
    [void]$problems.Add("server metadata must name one npm package")
}
elseif ($packages[0].version -ne $candidate.version) {
    [void]$problems.Add("npm package version $($packages[0].version) differs from candidate $($candidate.version)")
}

$values = @{}
if (Test-Path -LiteralPath $CredentialFile -PathType Leaf) {
    foreach ($line in Get-Content -LiteralPath $CredentialFile) {
        if ($line -match '^([A-Z0-9_]+)=(.*)$') {
            $values[$Matches[1]] = $Matches[2]
        }
    }
}
$keyPresent = $values.ContainsKey("MCP_DNS_PRIVATE_KEY") -and
    -not [string]::IsNullOrWhiteSpace($values.MCP_DNS_PRIVATE_KEY)
if (-not $keyPresent) {
    [void]$problems.Add("MCP_DNS_PRIVATE_KEY is missing")
}

Write-Output "MCP Registry action: $Action"
Write-Output "Server: $($server.name) $($server.version)"
Write-Output "Candidate: $($candidate.version) ($($candidate.status))"
Write-Output "Publisher: present"
Write-Output "DNS credential: $(if ($keyPresent) { 'present' } else { 'missing' })"
Write-Output "Readiness: $(if ($problems.Count -eq 0) { 'ready' } else { $problems -join '; ' })"
if ($Action -eq "Plan") {
    Write-Output "No npm or MCP Registry request was made."
    return
}
if ($problems.Count -gt 0) {
    throw "MCP Registry publication is not ready: $($problems -join '; ')"
}

& $publisher validate $ServerFile
if ($LASTEXITCODE -ne 0) {
    throw "mcp-publisher rejected $ServerFile"
}

$packageIdentifier = $packages[0].identifier
$packageVersion = $packages[0].version
$observedVersion = (& npm view "$packageIdentifier@$packageVersion" version).Trim()
if ($LASTEXITCODE -ne 0 -or $observedVersion -ne $packageVersion) {
    throw "The referenced npm package is not publicly observable at $packageVersion"
}

try {
    & $publisher login dns --domain sylin.org --private-key $values.MCP_DNS_PRIVATE_KEY
    if ($LASTEXITCODE -ne 0) {
        throw "MCP Registry DNS login failed"
    }
    & $publisher publish $ServerFile
    if ($LASTEXITCODE -ne 0) {
        throw "MCP Registry publication failed"
    }
}
finally {
    & $publisher logout *> $null
}
Write-Output "Published MCP Registry metadata: $($server.name) $($server.version)"
