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
use crate::events::{parse_timestamp, should_attach, sort_events, Event};
use crate::hashing::sha256_prefixed_hex;
use crate::signing::{build_envelope, OperatorKey, SignatureEnvelope};
use crate::telemetry::{CanonicalTelemetry, Location, RawInput, Source};
use crate::{Error, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

pub const CANONICAL_SCHEMA: &str = "ippan.pv.production.v1";
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

/// Compute the canonical record `Value` (sorted-keys is applied at hash time;
/// here we just build the logical content).
pub fn canonical_record_value(
    plant_id: &str,
    record_id: &str,
    timestamp: &str,
    interval_minutes: u32,
    source: &Source,
    location: &Location,
    canonical_telemetry: &CanonicalTelemetry,
    events: &[Event],
) -> Value {
    let mut tel = Map::new();
    tel.insert("ghi_w_m2".into(), json!(canonical_telemetry.ghi_w_m2));
    tel.insert(
        "ambient_temperature_milli_c".into(),
        json!(canonical_telemetry.ambient_temperature_milli_c),
    );
    tel.insert("dc_power_w".into(), json!(canonical_telemetry.dc_power_w));
    tel.insert("ac_power_w".into(), json!(canonical_telemetry.ac_power_w));
    tel.insert("meter_power_w".into(), json!(canonical_telemetry.meter_power_w));
    tel.insert(
        "performance_ratio_ppm".into(),
        json!(canonical_telemetry.performance_ratio_ppm),
    );
    tel.insert(
        "energy_since_start_wh".into(),
        json!(canonical_telemetry.energy_since_start_wh),
    );

    let events_arr: Vec<Value> = events
        .iter()
        .map(|e| {
            let mut m = Map::new();
            m.insert("event_id".into(), json!(e.event_id));
            m.insert("event_type".into(), json!(e.event_type));
            m.insert("started_at".into(), json!(e.started_at));
            if let Some(ref end) = e.ended_at {
                m.insert("ended_at".into(), json!(end));
            }
            m.insert("description".into(), json!(e.description));
            m.insert(
                "affected_components".into(),
                json!(e.affected_components),
            );
            m.insert("operator".into(), json!(e.operator));
            Value::Object(m)
        })
        .collect();

    let mut top = Map::new();
    top.insert("schema".into(), json!(CANONICAL_SCHEMA));
    top.insert("plant_id".into(), json!(plant_id));
    top.insert("record_id".into(), json!(record_id));
    top.insert("timestamp".into(), json!(timestamp));
    top.insert("interval_minutes".into(), json!(interval_minutes));

    let mut src = Map::new();
    src.insert("source_type".into(), json!(source.source_type));
    src.insert("source_id".into(), json!(source.source_id));
    top.insert("source".into(), Value::Object(src));

    let mut loc = Map::new();
    loc.insert("city".into(), json!(location.city));
    loc.insert("country".into(), json!(location.country));
    top.insert("location".into(), Value::Object(loc));

    top.insert("telemetry".into(), Value::Object(tel));
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
pub fn bundle_path(base_dir: &Path, plant_id: &str, ts: &DateTime<Utc>, record_id: &str) -> PathBuf {
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

    let canonical_tel = raw.telemetry.to_canonical()?;
    let ts = parse_timestamp(&raw.timestamp)?;
    let rid = record_id(&raw.plant_id, &ts);

    // Validate events, filter to attached, sort deterministically.
    let interval = Duration::minutes(raw.interval_minutes as i64);
    let interval_start = ts;
    let interval_end = ts + interval;

    let mut attached: Vec<Event> = Vec::new();
    for ev in events_input {
        ev.validate()?;
        if should_attach(ev, interval_start, interval_end, cfg.events.lookback_minutes)? {
            attached.push(ev.clone());
        }
    }
    sort_events(&mut attached);

    // Logical canonical record + canonical bytes for hashing.
    let record_val = canonical_record_value(
        &raw.plant_id,
        &rid,
        &raw.timestamp,
        raw.interval_minutes,
        &raw.source,
        &raw.location,
        &canonical_tel,
        &attached,
    );
    let canonical_bytes = to_canonical_bytes(&record_val)?;
    let canonical_hash = sha256_prefixed_hex(&canonical_bytes);

    // Signature envelope.
    let created_at = Utc::now()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let envelope = build_envelope(
        key,
        &rid,
        &raw.plant_id,
        &canonical_hash,
        &canonical_bytes,
        &created_at,
    );

    // On-disk layout.
    let base = Path::new(&cfg.storage.base_dir);
    let dir = bundle_path(base, &raw.plant_id, &ts, &rid);
    if dir.exists() && !opts.force {
        // Append-only: refuse to clobber a finished bundle. Existence of the
        // canonical-record file is the marker.
        let marker = dir.join(CANONICAL_FILE);
        if marker.exists() {
            return Err(Error::Bundle(format!(
                "bundle already exists at {} (use --force to overwrite)",
                dir.display()
            )));
        }
    }
    fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;

    // Write canonical-record.json as the exact canonical bytes — verification
    // re-hashes this file directly.
    atomic_write(&dir.join(CANONICAL_FILE), &canonical_bytes)?;

    // events.json — list of attached events, pretty-printed for human review.
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
        "source": {
            "source_type": raw.source.source_type,
            "source_id": raw.source.source_id,
        },
        "interval_window_minutes": opts.interval_window_minutes,
        "lookback_minutes": cfg.events.lookback_minutes,
        "created_at": created_at,
    });
    atomic_write(
        &dir.join(SOURCE_META_FILE),
        serde_json::to_vec_pretty(&source_meta)?.as_slice(),
    )?;

    // signature-envelope.json
    atomic_write(
        &dir.join(SIGNATURE_FILE),
        serde_json::to_vec_pretty(&envelope)?.as_slice(),
    )?;

    // anchor-request.json — commitment only, never telemetry.
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

    // anchor-response.json — placeholder until submission.
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

    // Manifest — file hashes for tamper detection.
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
