//! Evidence bundle assembly and on-disk layout.
//!
//! For every PV record we build a directory containing:
//!   - manifest.json
//!   - canonical-record.json   (the canonical record bytes)
//!   - signature-envelope.json
//!   - source-metadata.json
//!   - events.json
//!   - anchor-request.json
//!   - anchor-response.json    (may say "pending")
//!   - verification-report.json
//!
//! Writes are atomic (temp file + fsync + rename) and append-only by default.

use crate::canonical::to_canonical_bytes;
use crate::config::Config;
use crate::events::{parse_timestamp, should_attach, sort_events, AffectedComponent, Event};
use crate::hashing::sha256_prefixed_hex;
use crate::signing::{build_envelope, OperatorKey, SignatureEnvelope};
use crate::telemetry::{CanonicalTelemetry, Location, RawInput, Source};
use crate::{Error, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

pub const CANONICAL_SCHEMA: &str = "ippan.pv.production.v2";
pub const MANIFEST_SCHEMA: &str = "ippan.pv.evidence-manifest.v1";
pub const SOURCE_META_SCHEMA: &str = "ippan.pv.source-metadata.v1";
pub const EVENTS_FILE_SCHEMA: &str = "ippan.pv.events.v1";
pub const ANCHOR_REQ_SCHEMA: &str = "ippan.l1.anchor.request.v1";
pub const VERIFY_REPORT_SCHEMA: &str = "ippan.pv.verification-report.v1";

pub const CANONICAL_FILE: &str = "canonical-record.json";
pub const MANIFEST_FILE: &str = "manifest.json";
pub const SIGNATURE_FILE: &str = "signature-envelope.json";
pub const SOURCE_META_FILE: &str = "source-metadata.json";
pub const EVENTS_FILE: &str = "events.json";
pub const ANCHOR_REQ_FILE: &str = "anchor-request.json";
pub const ANCHOR_RESP_FILE: &str = "anchor-response.json";
pub const VERIFY_REPORT_FILE: &str = "verification-report.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub record_id: String,
    pub plant_id: String,
    pub created_at: String,
    pub canonical_hash: String,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorRequest {
    pub schema: String,
    pub workflow_type: String,
    pub operator_key_ref: String,
    pub evidence_bundle_id: String,
    pub commitment: Commitment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commitment {
    pub algorithm: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorResponsePending {
    pub status: String,
    pub note: String,
}

/// Full result of building one evidence bundle.
#[derive(Debug)]
pub struct BuiltBundle {
    pub record_id: String,
    pub canonical_hash: String,
    pub bundle_dir: PathBuf,
    pub envelope: SignatureEnvelope,
}

pub struct BuildOptions {
    pub force: bool,
    pub interval_window_minutes: i64,
}

impl Default for BuildOptions {
    fn default() -> Self {
        BuildOptions {
            force: false,
            interval_window_minutes: 15,
        }
    }
}

/// Format a record id from plant id + timestamp.
pub fn record_id(plant_id: &str, ts: &DateTime<Utc>) -> String {
    format!("pv-{}-{}", plant_id, ts.format("%Y%m%dT%H%M%SZ"))
}

fn affected_components_to_value(components: &[AffectedComponent]) -> Value {
    let arr: Vec<Value> = components
        .iter()
        .map(|c| {
            let mut m = Map::new();
            m.insert("type".into(), json!(c.kind));
            m.insert("id".into(), json!(c.id));
            m.insert("strings_offline".into(), json!(c.strings_offline));
            Value::Object(m)
        })
        .collect();
    Value::Array(arr)
}

fn event_to_value(e: &Event) -> Value {
    let mut m = Map::new();
    m.insert("event_id".into(), json!(e.event_id));
    m.insert("event_type".into(), json!(e.event_type));
    m.insert("started_at".into(), json!(e.started_at));
    if let Some(ref end) = e.ended_at {
        m.insert("ended_at".into(), json!(end));
    }
    m.insert("description".into(), json!(e.description));
    m.insert("status".into(), json!(e.status));
    m.insert(
        "affected_components".into(),
        affected_components_to_value(&e.affected_components),
    );
    if let Some(ref impact) = e.impact {
        let mut im = Map::new();
        if let Some(s) = impact.strings_offline {
            im.insert("strings_offline".into(), json!(s));
        }
        if let Some(p) = impact.power_reduction_pct {
            im.insert("power_reduction_pct".into(), json!(p));
        }
        if let Some(ref d) = impact.derating_at_start {
            // Preserve as a string — no float on the hashing path.
            im.insert("derating_at_start".into(), json!(d));
        }
        m.insert("impact".into(), Value::Object(im));
    }
    m.insert("operator".into(), json!(e.operator));
    if let Some(ref n) = e.notes {
        m.insert("notes".into(), json!(n));
    }
    if !e.photos.is_empty() {
        let photos: Vec<Value> = e
            .photos
            .iter()
            .map(|p| {
                let mut pm = Map::new();
                pm.insert("photo_id".into(), json!(p.photo_id));
                pm.insert("filename".into(), json!(p.filename));
                pm.insert("timestamp".into(), json!(p.timestamp));
                pm.insert("photo_type".into(), json!(p.photo_type));
                if let Some(ref d) = p.description {
                    pm.insert("description".into(), json!(d));
                }
                Value::Object(pm)
            })
            .collect();
        m.insert("photos".into(), Value::Array(photos));
    }
    if let Some(ref s) = e.root_cause {
        m.insert("root_cause".into(), json!(s));
    }
    if let Some(ref s) = e.spare_part {
        m.insert("spare_part".into(), json!(s));
    }
    if let Some(ref s) = e.insurance_claim {
        m.insert("insurance_claim".into(), json!(s));
    }
    if let Some(b) = e.soiling_reset {
        m.insert("soiling_reset".into(), json!(b));
    }
    Value::Object(m)
}

fn telemetry_to_value(t: &CanonicalTelemetry) -> Value {
    let mut tel = Map::new();
    tel.insert("ghi_w_m2".into(), json!(t.ghi_w_m2));
    tel.insert("dni_w_m2".into(), json!(t.dni_w_m2));
    tel.insert("dhi_w_m2".into(), json!(t.dhi_w_m2));
    tel.insert(
        "ambient_temperature_milli_c".into(),
        json!(t.ambient_temperature_milli_c),
    );
    tel.insert("humidity_milli_pct".into(), json!(t.humidity_milli_pct));
    tel.insert("wind_speed_milli_ms".into(), json!(t.wind_speed_milli_ms));
    tel.insert(
        "precipitation_milli_mm".into(),
        json!(t.precipitation_milli_mm),
    );
    tel.insert("cloudcover_milli_pct".into(), json!(t.cloudcover_milli_pct));
    tel.insert(
        "solar_elevation_milli_deg".into(),
        json!(t.solar_elevation_milli_deg),
    );
    tel.insert(
        "solar_azimuth_milli_deg".into(),
        json!(t.solar_azimuth_milli_deg),
    );
    tel.insert("poa_global_w_m2".into(), json!(t.poa_global_w_m2));
    tel.insert("poa_direct_w_m2".into(), json!(t.poa_direct_w_m2));
    tel.insert("poa_diffuse_w_m2".into(), json!(t.poa_diffuse_w_m2));
    tel.insert(
        "cell_temperature_milli_c".into(),
        json!(t.cell_temperature_milli_c),
    );
    tel.insert(
        "dc_string_voltage_milli_v".into(),
        json!(t.dc_string_voltage_milli_v),
    );
    tel.insert(
        "dc_string_current_milli_a".into(),
        json!(t.dc_string_current_milli_a),
    );
    tel.insert(
        "dc_array_voltage_milli_v".into(),
        json!(t.dc_array_voltage_milli_v),
    );
    tel.insert("dc_power_milliwatt".into(), json!(t.dc_power_milliwatt));
    tel.insert("ac_power_milliwatt".into(), json!(t.ac_power_milliwatt));
    tel.insert(
        "inverter_efficiency_milli_pct".into(),
        json!(t.inverter_efficiency_milli_pct),
    );
    tel.insert(
        "meter_power_milliwatt".into(),
        json!(t.meter_power_milliwatt),
    );
    tel.insert(
        "apparent_power_milli_va".into(),
        json!(t.apparent_power_milli_va),
    );
    tel.insert(
        "reactive_power_milli_var".into(),
        json!(t.reactive_power_milli_var),
    );
    tel.insert("grid_voltage_milli_v".into(), json!(t.grid_voltage_milli_v));
    tel.insert(
        "grid_frequency_milli_hz".into(),
        json!(t.grid_frequency_milli_hz),
    );
    tel.insert(
        "performance_ratio_ppm".into(),
        json!(t.performance_ratio_ppm),
    );
    tel.insert(
        "capacity_factor_milli_pct".into(),
        json!(t.capacity_factor_milli_pct),
    );
    tel.insert(
        "energy_since_start_wh".into(),
        json!(t.energy_since_start_wh),
    );
    tel.insert("strings_available".into(), json!(t.strings_available));
    tel.insert("derating_factor_ppm".into(), json!(t.derating_factor_ppm));
    tel.insert("soiling_factor_ppm".into(), json!(t.soiling_factor_ppm));
    Value::Object(tel)
}

fn source_to_value(s: &Source) -> Value {
    let mut m = Map::new();
    m.insert("source_type".into(), json!(s.source_type));
    m.insert("source_id".into(), json!(s.source_id));
    if let Some(ref v) = s.model {
        m.insert("model".into(), json!(v));
    }
    if let Some(ref v) = s.weather_provider {
        m.insert("weather_provider".into(), json!(v));
    }
    Value::Object(m)
}

fn location_to_value(l: &Location) -> Value {
    let mut m = Map::new();
    m.insert("city".into(), json!(l.city));
    m.insert("country".into(), json!(l.country));
    if let Some(ref v) = l.latitude {
        m.insert("latitude".into(), json!(v));
    }
    if let Some(ref v) = l.longitude {
        m.insert("longitude".into(), json!(v));
    }
    if let Some(ref v) = l.altitude_m {
        m.insert("altitude_m".into(), json!(v));
    }
    Value::Object(m)
}

/// Compute the canonical record `Value`. The canonical encoder sorts keys
/// recursively before hashing, so map insertion order here is informational
/// only — for human review.
#[allow(clippy::too_many_arguments)]
pub fn canonical_record_value(
    plant_id: &str,
    record_id: &str,
    timestamp: &str,
    interval_minutes: u32,
    source: &Source,
    location: &Location,
    canonical_telemetry: &CanonicalTelemetry,
    events: &[Event],
    active_event_ids: &[String],
) -> Value {
    let events_arr: Vec<Value> = events.iter().map(event_to_value).collect();

    // Sort active_event_ids deterministically for the canonical record.
    let mut active_sorted: Vec<String> = active_event_ids.to_vec();
    active_sorted.sort();
    active_sorted.dedup();

    let mut top = Map::new();
    top.insert("schema".into(), json!(CANONICAL_SCHEMA));
    top.insert("plant_id".into(), json!(plant_id));
    top.insert("record_id".into(), json!(record_id));
    top.insert("timestamp".into(), json!(timestamp));
    top.insert("interval_minutes".into(), json!(interval_minutes));
    top.insert("source".into(), source_to_value(source));
    top.insert("location".into(), location_to_value(location));
    top.insert("telemetry".into(), telemetry_to_value(canonical_telemetry));
    top.insert(
        "active_event_ids".into(),
        Value::Array(active_sorted.into_iter().map(Value::String).collect()),
    );
    top.insert("events".into(), Value::Array(events_arr));

    Value::Object(top)
}

/// Atomic write: temp file → fsync → rename → fsync parent.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| Error::io(&tmp, e))?;
        f.write_all(bytes).map_err(|e| Error::io(&tmp, e))?;
        let _ = f.sync_all();
    }
    fs::rename(&tmp, path).map_err(|e| Error::io(path, e))?;
    if let Some(parent) = path.parent() {
        if let Ok(d) = fs::File::open(parent) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

/// Resolve the bundle directory under base_dir.
pub fn bundle_path(
    base_dir: &Path,
    plant_id: &str,
    ts: &DateTime<Utc>,
    record_id: &str,
) -> PathBuf {
    base_dir
        .join(plant_id)
        .join("records")
        .join(ts.format("%Y").to_string())
        .join(ts.format("%m").to_string())
        .join(ts.format("%d").to_string())
        .join(record_id)
}

/// Build a complete evidence bundle from raw input + events + config + key.
pub fn build_bundle(
    cfg: &Config,
    raw: &RawInput,
    events_input: &[Event],
    key: &OperatorKey,
    opts: &BuildOptions,
) -> Result<BuiltBundle> {
    if raw.plant_id != cfg.agent.plant_id {
        return Err(Error::Bundle(format!(
            "raw.plant_id `{}` does not match config plant_id `{}`",
            raw.plant_id, cfg.agent.plant_id
        )));
    }

    // Telemetry → canonical integers (no float on the hashing path).
    let canonical_tel = raw.telemetry.to_canonical()?;
    let ts = parse_timestamp(&raw.timestamp)?;
    let rid = record_id(&raw.plant_id, &ts);

    // Validate every supplied event before touching disk.
    for ev in events_input {
        ev.validate()?;
    }

    // Cross-reference active_event_ids against the supplied events. When an
    // events list is provided, every active id MUST resolve to a known event.
    if !events_input.is_empty() {
        let known: BTreeSet<&str> = events_input.iter().map(|e| e.event_id.as_str()).collect();
        for active_id in &raw.active_event_ids {
            if !known.contains(active_id.as_str()) {
                return Err(Error::InvalidEvent(format!(
                    "active_event_ids references unknown event_id `{}` (not present in events.json)",
                    active_id
                )));
            }
        }
    }

    // Filter to attached, sort deterministically.
    let interval = Duration::minutes(raw.interval_minutes as i64);
    let interval_start = ts;
    let interval_end = ts + interval;
    let mut attached: Vec<Event> = Vec::new();
    let active_set: BTreeSet<&str> = raw.active_event_ids.iter().map(String::as_str).collect();
    for ev in events_input {
        let is_active = active_set.contains(ev.event_id.as_str());
        let attach = is_active
            || should_attach(
                ev,
                interval_start,
                interval_end,
                cfg.events.lookback_minutes,
            )?;
        if attach {
            attached.push(ev.clone());
        }
    }
    sort_events(&mut attached);

    let record_val = canonical_record_value(
        &raw.plant_id,
        &rid,
        &raw.timestamp,
        raw.interval_minutes,
        &raw.source,
        &raw.location,
        &canonical_tel,
        &attached,
        &raw.active_event_ids,
    );
    let canonical_bytes = to_canonical_bytes(&record_val)?;
    let canonical_hash = sha256_prefixed_hex(&canonical_bytes);

    let created_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let envelope = build_envelope(
        key,
        &rid,
        &raw.plant_id,
        &canonical_hash,
        &canonical_bytes,
        &created_at,
    );

    let base = Path::new(&cfg.storage.base_dir);
    let dir = bundle_path(base, &raw.plant_id, &ts, &rid);
    if dir.exists() && !opts.force {
        let marker = dir.join(CANONICAL_FILE);
        if marker.exists() {
            return Err(Error::Bundle(format!(
                "bundle already exists at {} (use --force to overwrite)",
                dir.display()
            )));
        }
    }
    fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;

    atomic_write(&dir.join(CANONICAL_FILE), &canonical_bytes)?;

    let events_doc = json!({
        "schema": EVENTS_FILE_SCHEMA,
        "record_id": rid,
        "plant_id": raw.plant_id,
        "events": attached,
    });
    atomic_write(
        &dir.join(EVENTS_FILE),
        serde_json::to_vec_pretty(&events_doc)?.as_slice(),
    )?;

    // source-metadata.json — context that is NOT on the canonical hashing path.
    let source_meta = json!({
        "schema": SOURCE_META_SCHEMA,
        "record_id": rid,
        "agent_id": cfg.agent.agent_id,
        "agent_type": cfg.agent.agent_type,
        "operator_key_ref": cfg.agent.operator_key_ref,
        "source": source_to_value(&raw.source),
        "location": location_to_value(&raw.location),
        "active_event_ids": raw.active_event_ids,
        "interval_window_minutes": opts.interval_window_minutes,
        "lookback_minutes": cfg.events.lookback_minutes,
        "created_at": created_at,
    });
    atomic_write(
        &dir.join(SOURCE_META_FILE),
        serde_json::to_vec_pretty(&source_meta)?.as_slice(),
    )?;

    atomic_write(
        &dir.join(SIGNATURE_FILE),
        serde_json::to_vec_pretty(&envelope)?.as_slice(),
    )?;

    let anchor_req = AnchorRequest {
        schema: ANCHOR_REQ_SCHEMA.into(),
        workflow_type: "pv_production_evidence".into(),
        operator_key_ref: cfg.agent.operator_key_ref.clone(),
        evidence_bundle_id: rid.clone(),
        commitment: Commitment {
            algorithm: "sha256".into(),
            hash: canonical_hash.clone(),
        },
    };
    atomic_write(
        &dir.join(ANCHOR_REQ_FILE),
        serde_json::to_vec_pretty(&anchor_req)?.as_slice(),
    )?;

    if !dir.join(ANCHOR_RESP_FILE).exists() {
        let pending = AnchorResponsePending {
            status: "pending".into(),
            note: "Anchor has not been submitted yet. Run `pv-agent anchor-submit` when ready."
                .into(),
        };
        atomic_write(
            &dir.join(ANCHOR_RESP_FILE),
            serde_json::to_vec_pretty(&pending)?.as_slice(),
        )?;
    }

    let manifest_files = [
        CANONICAL_FILE,
        SIGNATURE_FILE,
        SOURCE_META_FILE,
        EVENTS_FILE,
        ANCHOR_REQ_FILE,
    ];
    let mut files = Vec::new();
    for name in manifest_files {
        let p = dir.join(name);
        let bytes = fs::read(&p).map_err(|e| Error::io(&p, e))?;
        files.push(ManifestFile {
            path: name.to_string(),
            sha256: sha256_prefixed_hex(&bytes),
        });
    }
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA.into(),
        record_id: rid.clone(),
        plant_id: raw.plant_id.clone(),
        created_at: created_at.clone(),
        canonical_hash: canonical_hash.clone(),
        files,
    };
    atomic_write(
        &dir.join(MANIFEST_FILE),
        serde_json::to_vec_pretty(&manifest)?.as_slice(),
    )?;

    Ok(BuiltBundle {
        record_id: rid,
        canonical_hash,
        bundle_dir: dir,
        envelope,
    })
}

/// Load a bundle's canonical-record bytes and signature envelope from disk.
pub fn read_canonical_bytes(bundle_dir: &Path) -> Result<Vec<u8>> {
    let p = bundle_dir.join(CANONICAL_FILE);
    fs::read(&p).map_err(|e| Error::io(&p, e))
}

pub fn read_manifest(bundle_dir: &Path) -> Result<Manifest> {
    let p = bundle_dir.join(MANIFEST_FILE);
    let bytes = fs::read(&p).map_err(|e| Error::io(&p, e))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn read_envelope(bundle_dir: &Path) -> Result<SignatureEnvelope> {
    let p = bundle_dir.join(SIGNATURE_FILE);
    let bytes = fs::read(&p).map_err(|e| Error::io(&p, e))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn read_anchor_request(bundle_dir: &Path) -> Result<AnchorRequest> {
    let p = bundle_dir.join(ANCHOR_REQ_FILE);
    let bytes = fs::read(&p).map_err(|e| Error::io(&p, e))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn read_anchor_response_raw(bundle_dir: &Path) -> Result<Value> {
    let p = bundle_dir.join(ANCHOR_RESP_FILE);
    let bytes = fs::read(&p).map_err(|e| Error::io(&p, e))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn write_anchor_response(bundle_dir: &Path, response: &Value) -> Result<()> {
    atomic_write(
        &bundle_dir.join(ANCHOR_RESP_FILE),
        serde_json::to_vec_pretty(response)?.as_slice(),
    )
}

pub fn write_verification_report(bundle_dir: &Path, report: &Value) -> Result<()> {
    atomic_write(
        &bundle_dir.join(VERIFY_REPORT_FILE),
        serde_json::to_vec_pretty(report)?.as_slice(),
    )
}
