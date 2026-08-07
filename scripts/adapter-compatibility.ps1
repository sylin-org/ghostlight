# Shared Chrome-adapter compatibility helpers for release and public-surface checks.

function ConvertTo-GhostlightVersion([string] $Value, [string] $Label) {
    if ($Value -notmatch '^\d+\.\d+\.\d+$') {
        throw "$Label '$Value' is not semantic x.y.z"
    }
    return [version]$Value
}

function ConvertTo-GhostlightVersionBlock([string] $Value, [string] $Label) {
    if ($Value -notmatch '^\d+\.\d+$') {
        throw "$Label '$Value' is not a semantic major.minor block"
    }
    return $Value
}

function Read-GhostlightAdapterCompatibility([string] $Root) {
    $path = Join-Path $Root 'compatibility.json'
    if (-not (Test-Path -LiteralPath $path)) {
        throw "compatibility.json is missing at $path"
    }

    $map = Get-Content -Raw -LiteralPath $path | ConvertFrom-Json
    if ($map.schemaVersion -ne 2) {
        throw "compatibility.json has unsupported schemaVersion '$($map.schemaVersion)'"
    }

    $entries = @($map.chromeAdapters)
    if ($entries.Count -eq 0) {
        throw 'compatibility.json has no chromeAdapters entries'
    }

    $seen = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($entry in $entries) {
        $adapter = ConvertTo-GhostlightVersion $entry.adapterVersion 'Chrome adapter version'
        $hasBlock = $null -ne $entry.PSObject.Properties['serviceVersionBlock']
        $hasMinimum = $null -ne $entry.PSObject.Properties['minimumServiceVersion']
        $hasMaximum = $null -ne $entry.PSObject.Properties['maximumServiceVersion']
        if ($hasBlock -eq ($hasMinimum -or $hasMaximum)) {
            throw "Chrome adapter $adapter must declare exactly one compatibility contract"
        }
        if ($hasBlock) {
            [void](ConvertTo-GhostlightVersionBlock `
                $entry.serviceVersionBlock 'service version block')
        }
        else {
            if (-not ($hasMinimum -and $hasMaximum)) {
                throw "Chrome adapter $adapter must declare both ends of its service range"
            }
            $minimum = ConvertTo-GhostlightVersion `
                $entry.minimumServiceVersion 'minimum service version'
            $maximum = ConvertTo-GhostlightVersion `
                $entry.maximumServiceVersion 'maximum service version'
            if ($minimum -gt $maximum) {
                throw "Chrome adapter $adapter has an inverted service range: $minimum-$maximum"
            }
        }
        if (-not $seen.Add($entry.adapterVersion)) {
            throw "compatibility.json repeats Chrome adapter $($entry.adapterVersion)"
        }
    }

    return $map
}

function Get-GhostlightChromeAdapterVersion([string] $Root) {
    $path = Join-Path $Root 'extension/manifest.json'
    $manifest = Get-Content -Raw -LiteralPath $path | ConvertFrom-Json
    [void](ConvertTo-GhostlightVersion $manifest.version 'extension manifest version')
    return $manifest.version
}

function Get-GhostlightChromeAdapterCoverage($Map, [string] $AdapterVersion) {
    $matches = @($Map.chromeAdapters | Where-Object { $_.adapterVersion -eq $AdapterVersion })
    if ($matches.Count -ne 1) {
        throw "Chrome adapter $AdapterVersion has no unique compatibility entry"
    }
    return $matches[0]
}

function Assert-GhostlightChromeAdapterCoversService(
    $Map,
    [string] $AdapterVersion,
    [string] $ServiceVersion
) {
    $coverage = Get-GhostlightChromeAdapterCoverage $Map $AdapterVersion
    $service = ConvertTo-GhostlightVersion $ServiceVersion 'service version'
    if ($null -ne $coverage.PSObject.Properties['serviceVersionBlock']) {
        $block = ConvertTo-GhostlightVersionBlock `
            $coverage.serviceVersionBlock 'service version block'
        $actual = "$($service.Major).$($service.Minor)"
        if ($actual -ne $block) {
            throw "Chrome adapter $AdapterVersion covers service versions $block.x, not $service"
        }
        return $coverage
    }

    $minimum = ConvertTo-GhostlightVersion $coverage.minimumServiceVersion 'minimum service version'
    $maximum = ConvertTo-GhostlightVersion $coverage.maximumServiceVersion 'maximum service version'
    if ($service -lt $minimum -or $service -gt $maximum) {
        throw "Chrome adapter $AdapterVersion covers service versions $minimum-$maximum, not $service"
    }
    return $coverage
}

function Format-GhostlightChromeAdapterCoverage($Coverage) {
    if ($null -ne $Coverage.PSObject.Properties['serviceVersionBlock']) {
        return "Chrome adapter v$($Coverage.adapterVersion) covers Ghostlight service versions " +
            "v$($Coverage.serviceVersionBlock).x."
    }
    return "Chrome adapter v$($Coverage.adapterVersion) covers Ghostlight service versions " +
        "v$($Coverage.minimumServiceVersion)-v$($Coverage.maximumServiceVersion)."
}

function Get-GhostlightChromeStorePublicVersion([string] $ItemId) {
    if ($ItemId -notmatch '^[a-p]{32}$') {
        throw "Chrome Web Store item id '$ItemId' is not a 32-character extension id"
    }

    $encoded = [Uri]::EscapeDataString("id=$ItemId&uc")
    $uri = "https://clients2.google.com/service/update2/crx?response=updatecheck" +
        "&prodversion=999.0.0.0&acceptformat=crx2,crx3&x=$encoded"
    $response = Invoke-WebRequest -UseBasicParsing -Uri $uri -TimeoutSec 20
    if ($response.StatusCode -ne 200) {
        throw "Chrome Web Store update endpoint returned HTTP $($response.StatusCode)"
    }

    [xml] $document = $response.Content
    $update = $document.SelectSingleNode(
        "//*[local-name()='app' and @appid='$ItemId']/*[local-name()='updatecheck']"
    )
    if (-not $update -or $update.status -ne 'ok') {
        throw "Chrome Web Store update endpoint returned no public version for $ItemId"
    }

    $version = [string] $update.version
    [void](ConvertTo-GhostlightVersion $version 'public Chrome adapter version')
    return $version
}

function Format-GhostlightExtensionSummary(
    $Map,
    [string] $PublicServiceVersion,
    [string] $PublicAdapterVersion,
    [string] $PendingAdapterVersion = '',
    [string] $CandidateServiceVersion = ''
) {
    $publicCoverage = Assert-GhostlightChromeAdapterCoversService `
        $Map $PublicAdapterVersion $PublicServiceVersion
    $parts = [System.Collections.Generic.List[string]]::new()
    $parts.Add("The Chrome Web Store serves Chrome adapter v$PublicAdapterVersion.")
    $parts.Add((Format-GhostlightChromeAdapterCoverage $publicCoverage))

    if (-not [string]::IsNullOrWhiteSpace($PendingAdapterVersion)) {
        $pendingCoverage = Get-GhostlightChromeAdapterCoverage $Map $PendingAdapterVersion
        if (-not [string]::IsNullOrWhiteSpace($CandidateServiceVersion)) {
            [void](Assert-GhostlightChromeAdapterCoversService `
                $Map $PendingAdapterVersion $CandidateServiceVersion)
        }
        $parts.Add("Chrome adapter v$PendingAdapterVersion is pending review.")
        $parts.Add((Format-GhostlightChromeAdapterCoverage $pendingCoverage))
    }

    $parts.Add('Install the extension from the public listing.')
    return $parts -join ' '
}
