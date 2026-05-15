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
    }
    if let Some(src) = canonical.get("source") {
        if let (Some(t), Some(i)) = (
            src.get("source_type").and_then(|v| v.as_str()),
            src.get("source_id").and_then(|v| v.as_str()),
        ) {
            lines.push(format!("Source:           {} / {}", t, i));
        }
    }

    if let Some(tel) = canonical.get("telemetry") {
        lines.push(String::new());
        lines.push("Telemetry (canonical, integer):".into());
        for k in [
            "ghi_w_m2",
            "ambient_temperature_milli_c",
            "dc_power_w",
            "ac_power_w",
            "meter_power_w",
            "performance_ratio_ppm",
            "energy_since_start_wh",
        ] {
            if let Some(v) = tel.get(k).and_then(|v| v.as_i64()) {
                lines.push(format!("  {:30} {}", k, v));
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
            lines.push(format!("  - {} [{}] started {}", id, t, st));
        }
    }

    lines.push(String::new());
    lines.push(format!("Canonical hash:   {}", manifest.canonical_hash));
    lines.push(format!(
        "Signature:        algorithm={} key_ref={}",
        envelope.signature.algorithm, envelope.signature.operator_key_ref
    ));
    // We deliberately print only the key_ref, NOT the signature bytes,
    // public key, or any token.

    // Anchor status — no token, no header.
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
