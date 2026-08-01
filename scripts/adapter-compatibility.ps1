# Shared Chrome-adapter compatibility helpers for release and public-surface checks.

function ConvertTo-GhostlightVersion([string] $Value, [string] $Label) {
    if ($Value -notmatch '^\d+\.\d+\.\d+$') {
        throw "$Label '$Value' is not semantic x.y.z"
    }
    return [version]$Value
}

function Read-GhostlightAdapterCompatibility([string] $Root) {
    $path = Join-Path $Root 'compatibility.json'
    if (-not (Test-Path -LiteralPath $path)) {
        throw "compatibility.json is missing at $path"
    }

    $map = Get-Content -Raw -LiteralPath $path | ConvertFrom-Json
    if ($map.schemaVersion -ne 1) {
        throw "compatibility.json has unsupported schemaVersion '$($map.schemaVersion)'"
    }

    $entries = @($map.chromeAdapters)
    if ($entries.Count -eq 0) {
        throw 'compatibility.json has no chromeAdapters entries'
    }

    $seen = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($entry in $entries) {
        $adapter = ConvertTo-GhostlightVersion $entry.adapterVersion 'Chrome adapter version'
        $minimum = ConvertTo-GhostlightVersion $entry.minimumServiceVersion 'minimum service version'
        $maximum = ConvertTo-GhostlightVersion $entry.maximumServiceVersion 'maximum service version'
        if ($minimum -gt $maximum) {
            throw "Chrome adapter $adapter has an inverted service range: $minimum-$maximum"
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
    $minimum = ConvertTo-GhostlightVersion $coverage.minimumServiceVersion 'minimum service version'
    $maximum = ConvertTo-GhostlightVersion $coverage.maximumServiceVersion 'maximum service version'
    if ($service -lt $minimum -or $service -gt $maximum) {
        throw "Chrome adapter $AdapterVersion covers service versions $minimum-$maximum, not $service"
    }
    return $coverage
}

function Format-GhostlightChromeAdapterCoverage($Coverage) {
    return "Chrome adapter v$($Coverage.adapterVersion) covers Ghostlight service versions " +
        "v$($Coverage.minimumServiceVersion)-v$($Coverage.maximumServiceVersion)."
}
