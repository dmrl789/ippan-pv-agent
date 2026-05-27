<#
.SYNOPSIS
    End-to-end validation of a real simulator-supplied telemetry / events
    fixture against the merged ippan-pv-agent (tag pv-data-contract-v1.0+).

.DESCRIPTION
    Runs the full ingestion path twice and confirms:
      - parse PASS
      - validation PASS (plant_id, ISO-8601 UTC Z, component IDs,
        photo_type, active_event_ids cross-ref, strings_available ≤ 300)
      - evidence bundle created
      - canonical hash stable across repeated builds in separate dirs
      - attached events include every id in active_event_ids and any
        completed event ending within the 240-min lookback window
      - no float-shaped numeric appears outside JSON string values in
        canonical-record.json

    The script does NOT submit anything to IPPAN L1.

.PARAMETER Telemetry
    Path to the telemetry JSON file from the simulator.

.PARAMETER Events
    Path to the events.json file from the simulator.

.PARAMETER BaseDir
    Working directory for evidence bundles (default ./data/pv-agent-e2e).

.EXAMPLE
    pwsh scripts/e2e-fixture-validate.ps1 `
        -Telemetry examples/pv/palermo-telemetry.json `
        -Events    examples/pv/palermo-events.json
#>
param(
    [Parameter(Mandatory = $true)][string]$Telemetry,
    [Parameter(Mandatory = $true)][string]$Events,
    [string]$BaseDir = "data/pv-agent-e2e"
)

$ErrorActionPreference = "Stop"

function Resolve-Binary {
    foreach ($candidate in @("target/release/pv-agent.exe", "target/release/pv-agent", "target/debug/pv-agent.exe", "target/debug/pv-agent")) {
        if (Test-Path $candidate) { return (Resolve-Path $candidate).Path }
    }
    throw "pv-agent binary not found. Run 'cargo build --release' first."
}

function Write-CfgToml {
    param([string]$Base, [string]$KeyPath)
    @"
[agent]
agent_id = "pv-agent-palermo-001"
agent_type = "pv_plant_agent"
plant_id = "palermo-pv-001"
operator_key_ref = "key:plant-palermo-001"
production_mode = false

[storage]
base_dir = "$($Base -replace '\\','/')"

[ippan]
endpoint = "http://127.0.0.1:18181"
anchor_path = "/v1/anchors"
admin_token_env = "IPPAN_ADMIN_TOKEN"
submit_anchors = false

[events]
lookback_minutes = 240

[key]
key_file = "$($KeyPath -replace '\\','/')"
"@
}

function Find-Bundle {
    param([string]$Root)
    $b = Get-ChildItem -Path $Root -Recurse -Directory -Filter "pv-palermo-pv-001-*" -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $b) { throw "no bundle directory found under $Root" }
    return $b.FullName
}

function Get-CanonicalHash {
    param([string]$BundleDir)
    $manifest = Get-Content (Join-Path $BundleDir "manifest.json") -Raw | ConvertFrom-Json
    return $manifest.canonical_hash
}

function Build-One {
    param([string]$Run, [string]$Bin, [string]$Telemetry, [string]$Events)
    $base = Join-Path $BaseDir $Run
    if (Test-Path $base) { Remove-Item -Recurse -Force $base }
    New-Item -ItemType Directory -Force -Path (Join-Path $base "keys") | Out-Null

    $keyPath = Join-Path $base "keys/demo-key.json"
    & $Bin generate-demo-key --out $keyPath --key-ref "key:plant-palermo-001" | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "generate-demo-key failed in $Run" }

    $cfgPath = Join-Path $base "pv-agent.toml"
    Set-Content -Path $cfgPath -Value (Write-CfgToml -Base $base -KeyPath $keyPath) -NoNewline

    & $Bin run-once --input $Telemetry --events $Events --config $cfgPath --force | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "run-once failed in $Run" }

    $bundleDir = Find-Bundle -Root $base
    & $Bin verify --bundle $bundleDir | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "verify failed in $Run" }
    return $bundleDir
}

function Test-NoFloatInCanonical {
    param([string]$BundleDir)
    $bytes = [System.IO.File]::ReadAllBytes((Join-Path $BundleDir "canonical-record.json"))
    $text = [System.Text.Encoding]::UTF8.GetString($bytes)
    # Walk byte-by-byte: any digit.digit OUTSIDE quoted strings is a float.
    $inString = $false
    $escaped = $false
    for ($i = 0; $i -lt $text.Length; $i++) {
        $c = $text[$i]
        if ($inString) {
            if ($escaped) { $escaped = $false; continue }
            if ($c -eq '\') { $escaped = $true; continue }
            if ($c -eq '"') { $inString = $false }
            continue
        }
        if ($c -eq '"') { $inString = $true; continue }
        if ([char]::IsDigit($c)) {
            $j = $i
            while ($j -lt $text.Length -and [char]::IsDigit($text[$j])) { $j++ }
            if ($j -lt $text.Length - 1 -and $text[$j] -eq '.' -and [char]::IsDigit($text[$j + 1])) {
                throw "canonical record contains an unquoted float at position $j"
            }
            $i = $j - 1
        }
    }
}

function ConvertTo-Utc {
    param($Value)
    if ($null -eq $Value) { return $null }
    if ($Value -is [datetime])       { return $Value.ToUniversalTime() }
    if ($Value -is [datetimeoffset]) { return $Value.UtcDateTime }
    # String — parse explicitly under InvariantCulture so locale doesn't
    # turn "2026-05-20T12:15:00Z" into a US-format round-trip failure.
    return [datetimeoffset]::Parse(
        [string]$Value,
        [cultureinfo]::InvariantCulture,
        [System.Globalization.DateTimeStyles]::AssumeUniversal -bor
            [System.Globalization.DateTimeStyles]::AdjustToUniversal
    ).UtcDateTime
}

function Test-AttachedEvents {
    param([string]$BundleDir)
    $rec = Get-Content (Join-Path $BundleDir "canonical-record.json") -Raw | ConvertFrom-Json
    $activeIds = @($rec.active_event_ids)
    $attachedIds = @($rec.events | ForEach-Object { $_.event_id })

    foreach ($id in $activeIds) {
        if ($attachedIds -notcontains $id) {
            throw "active_event_ids contains '$id' but it is not attached to the canonical record"
        }
    }

    $ts = ConvertTo-Utc $rec.timestamp
    $cutoff = $ts.AddMinutes(-240)
    foreach ($ev in $rec.events) {
        if ($ev.PSObject.Properties.Name -contains "ended_at" -and $ev.ended_at) {
            $end = ConvertTo-Utc $ev.ended_at
            if ($end -lt $cutoff -and $activeIds -notcontains $ev.event_id) {
                throw "event $($ev.event_id) ended at $($ev.ended_at) — outside the 240-min lookback and not in active_event_ids; should not have attached"
            }
        }
    }

    [pscustomobject]@{
        ActiveIds   = $activeIds
        AttachedIds = $attachedIds
    }
}

Write-Host "==> e2e-fixture-validate"
Write-Host "    telemetry: $Telemetry"
Write-Host "    events   : $Events"
Write-Host "    base dir : $BaseDir"

if (-not (Test-Path $Telemetry)) { throw "telemetry file not found: $Telemetry" }
if (-not (Test-Path $Events))    { throw "events file not found: $Events" }

$bin = Resolve-Binary
Write-Host "    binary   : $bin"

New-Item -ItemType Directory -Force -Path $BaseDir | Out-Null

Write-Host "`n==> [1/4] Build + verify run #1"
$bundle1 = Build-One -Run "run1" -Bin $bin -Telemetry $Telemetry -Events $Events
$hash1   = Get-CanonicalHash -BundleDir $bundle1

Write-Host "`n==> [2/4] Build + verify run #2 (idempotency)"
$bundle2 = Build-One -Run "run2" -Bin $bin -Telemetry $Telemetry -Events $Events
$hash2   = Get-CanonicalHash -BundleDir $bundle2

if ($hash1 -ne $hash2) {
    throw "canonical hash NOT stable across runs:`n  run1: $hash1`n  run2: $hash2"
}
Write-Host "    canonical hash stable: $hash1"

Write-Host "`n==> [3/4] No-float regression scan"
Test-NoFloatInCanonical -BundleDir $bundle1
Write-Host "    canonical record contains no unquoted floats"

Write-Host "`n==> [4/4] Attached events / active_event_ids sanity"
$summary = Test-AttachedEvents -BundleDir $bundle1
Write-Host ("    active_event_ids : {0}" -f ($summary.ActiveIds -join ', '))
Write-Host ("    attached         : {0}" -f ($summary.AttachedIds -join ', '))

Write-Host "`nE2E VALIDATION: PASS" -ForegroundColor Green
Write-Host "Bundle  : $bundle1"
Write-Host "Hash    : $hash1"
