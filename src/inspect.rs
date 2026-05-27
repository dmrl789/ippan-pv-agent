//! Human-readable bundle inspection. NEVER prints secrets, tokens, or
//! private keys.

use crate::bundle::{read_anchor_response_raw, read_envelope, read_manifest};
use crate::Result;
use serde_json::Value;
use std::fs;
use std::path::Path;

pub struct InspectReport {
    pub lines: Vec<String>,
}

/// Telemetry field names (in canonical integer form) printed by `inspect`.
/// Listed in display order — not alphabetical.
const CANONICAL_TELEMETRY_FIELDS: &[&str] = &[
    "ghi_w_m2",
    "dni_w_m2",
    "dhi_w_m2",
    "poa_global_w_m2",
    "poa_direct_w_m2",
    "poa_diffuse_w_m2",
    "ambient_temperature_milli_c",
    "cell_temperature_milli_c",
    "humidity_milli_pct",
    "wind_speed_milli_ms",
    "precipitation_milli_mm",
    "cloudcover_milli_pct",
    "solar_elevation_milli_deg",
    "solar_azimuth_milli_deg",
    "dc_string_voltage_milli_v",
    "dc_string_current_milli_a",
    "dc_array_voltage_milli_v",
    "dc_power_milliwatt",
    "ac_power_milliwatt",
    "inverter_efficiency_milli_pct",
    "meter_power_milliwatt",
    "apparent_power_milli_va",
    "reactive_power_milli_var",
    "grid_voltage_milli_v",
    "grid_frequency_milli_hz",
    "performance_ratio_ppm",
    "capacity_factor_milli_pct",
    "energy_since_start_wh",
    "strings_available",
    "derating_factor_ppm",
    "soiling_factor_ppm",
];

pub fn inspect(bundle_dir: &Path) -> Result<InspectReport> {
    let mut lines = Vec::new();

    let manifest = read_manifest(bundle_dir)?;
    let envelope = read_envelope(bundle_dir)?;
    let canonical_path = bundle_dir.join(crate::bundle::CANONICAL_FILE);
    let canonical_bytes = fs::read(&canonical_path)?;
    let canonical: Value = serde_json::from_slice(&canonical_bytes)?;

    lines.push(format!("Bundle:           {}", bundle_dir.display()));
    lines.push(format!("Plant ID:         {}", manifest.plant_id));
    lines.push(format!("Record ID:        {}", manifest.record_id));
    if let Some(ts) = canonical.get("timestamp").and_then(|v| v.as_str()) {
        lines.push(format!("Timestamp:        {}", ts));
    }
    if let Some(im) = canonical.get("interval_minutes").and_then(|v| v.as_u64()) {
        lines.push(format!("Interval:         {} minutes", im));
    }
    if let Some(loc) = canonical.get("location") {
        if let (Some(c), Some(co)) = (
            loc.get("city").and_then(|v| v.as_str()),
            loc.get("country").and_then(|v| v.as_str()),
        ) {
            lines.push(format!("Location:         {}, {}", c, co));
        }
        if let (Some(lat), Some(lon)) = (
            loc.get("latitude").and_then(|v| v.as_str()),
            loc.get("longitude").and_then(|v| v.as_str()),
        ) {
            lines.push(format!("Coordinates:      {}°, {}°", lat, lon));
        }
        if let Some(alt) = loc.get("altitude_m").and_then(|v| v.as_str()) {
            lines.push(format!("Altitude:         {} m", alt));
        }
    }
    if let Some(src) = canonical.get("source") {
        if let (Some(t), Some(i)) = (
            src.get("source_type").and_then(|v| v.as_str()),
            src.get("source_id").and_then(|v| v.as_str()),
        ) {
            lines.push(format!("Source:           {} / {}", t, i));
        }
        if let Some(m) = src.get("model").and_then(|v| v.as_str()) {
            lines.push(format!("Source model:     {}", m));
        }
        if let Some(w) = src.get("weather_provider").and_then(|v| v.as_str()) {
            lines.push(format!("Weather provider: {}", w));
        }
    }

    if let Some(active) = canonical.get("active_event_ids").and_then(|v| v.as_array()) {
        if !active.is_empty() {
            let ids: Vec<String> = active
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            lines.push(format!("Active events:    {}", ids.join(", ")));
        }
    }

    if let Some(tel) = canonical.get("telemetry") {
        lines.push(String::new());
        lines.push("Telemetry (canonical, integer):".into());
        for k in CANONICAL_TELEMETRY_FIELDS {
            if let Some(v) = tel.get(*k).and_then(|v| v.as_i64()) {
                lines.push(format!("  {:32} {}", k, v));
            }
        }
    }

    if let Some(events) = canonical.get("events").and_then(|v| v.as_array()) {
        lines.push(String::new());
        lines.push(format!("Attached events:  {}", events.len()));
        for ev in events {
            let id = ev.get("event_id").and_then(|v| v.as_str()).unwrap_or("?");
            let t = ev.get("event_type").and_then(|v| v.as_str()).unwrap_or("?");
            let st = ev.get("started_at").and_then(|v| v.as_str()).unwrap_or("?");
            let status = ev.get("status").and_then(|v| v.as_str()).unwrap_or("?");
            lines.push(format!(
                "  - {} [{}] started {} status={}",
                id, t, st, status
            ));
        }
    }

    lines.push(String::new());
    lines.push(format!("Canonical hash:   {}", manifest.canonical_hash));
    lines.push(format!(
        "Signature:        algorithm={} key_ref={}",
        envelope.signature.algorithm, envelope.signature.operator_key_ref
    ));

    let resp = read_anchor_response_raw(bundle_dir)?;
    let status = resp
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    lines.push(format!("Anchor status:    {}", status));
    if let Some(r) = resp.get("reference").and_then(|v| v.as_str()) {
        lines.push(format!("Anchor reference: {}", r));
    }

    Ok(InspectReport { lines })
}
