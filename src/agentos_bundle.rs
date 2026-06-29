//! AgentOS-compatible evidence bundle export (`ippan.pv.evidence_bundle.v1`).
//!
//! Reads an already-built local evidence bundle directory (manifest +
//! signature envelope + canonical record + source metadata + optional anchor
//! response) and assembles a single JSON object matching the import contract
//! implemented by IPPAN AgentOS Energy:
//!
//!   POST /api/energy/import-pv-agent-bundle  (validate + preview)
//!
//! The bundle carries hashes and the Ed25519 *signature over the canonical
//! record bytes* (the same signature the agent already produces). AgentOS
//! reports that signature as `present_not_verified` because it does not have
//! the original canonical bytes in this manifest; the `files.signed_payload`
//! pointer names which file holds them, for a future verification path. No raw
//! high-frequency telemetry rows are included. Nothing is submitted to L1.

use crate::{Error, Result};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub const AGENTOS_BUNDLE_SCHEMA: &str = "ippan.pv.evidence_bundle.v1";
const CANONICALIZATION: &str = "ippan.pv.canonical (sorted-keys, no-floats, utf-8)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceBundleV1 {
    pub schema_version: String,
    pub agent: AgentBlock,
    pub plant: PlantBlock,
    pub period: PeriodBlock,
    pub summary: SummaryBlock,
    pub hashes: HashesBlock,
    pub signature: SignatureBlock,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<AnchorBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<FilesBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBlock {
    pub agent_id: String,
    pub agent_version: String,
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantBlock {
    pub asset_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_label: Option<String>,
    pub technology: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodBlock {
    pub start: String,
    pub end: String,
    pub interval_minutes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryBlock {
    pub record_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_energy_kwh: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curtailment_kwh: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomalies_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashesBlock {
    pub data_hash: String,
    pub commitment_hash: String,
    pub canonicalization: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureBlock {
    pub algorithm: String,
    pub signer: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorBlock {
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesBlock {
    pub manifest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_report: Option<String>,
    /// Pointer to the exact bytes the signature was computed over, so a future
    /// AgentOS verification path can verify it. AgentOS does not verify today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_payload: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Pseudonymous, non-reversible asset reference. Mirrors the AgentOS
/// `pseudonymousAssetRef` derivation so the real plant id is never published.
pub fn pseudonymous_asset_ref(plant_id: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"ippan-energy-asset:");
    h.update(plant_id.as_bytes());
    let hex = hex::encode(h.finalize());
    format!("earef:{}", &hex[..32])
}

fn read_json(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).map_err(|e| Error::io(path, e))?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn require_str(v: &Value, ptr: &str, file: &str) -> Result<String> {
    v.pointer(ptr)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::Bundle(format!("missing `{ptr}` in {file}")))
}

fn opt_str(v: &Value, ptr: &str) -> Option<String> {
    v.pointer(ptr).and_then(Value::as_str).map(str::to_string)
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Build an AgentOS-compatible `ippan.pv.evidence_bundle.v1` from a local
/// evidence bundle directory. `agent_version` is normally the pv-agent crate
/// version (`env!("CARGO_PKG_VERSION")`).
pub fn build_agentos_bundle(bundle_dir: &Path, agent_version: &str) -> Result<EvidenceBundleV1> {
    let manifest = read_json(&bundle_dir.join("manifest.json"))?;
    let envelope = read_json(&bundle_dir.join("signature-envelope.json"))?;
    let record = read_json(&bundle_dir.join("canonical-record.json"))?;
    let source_meta = read_json(&bundle_dir.join("source-metadata.json"))?;

    let canonical_hash = require_str(&manifest, "/canonical_hash", "manifest.json")?;
    let plant_id = require_str(&manifest, "/plant_id", "manifest.json")?;

    // agent
    let agent_id = require_str(&source_meta, "/agent_id", "source-metadata.json")?;
    let public_key = require_str(
        &envelope,
        "/signature/public_key_b64",
        "signature-envelope.json",
    )?;

    // plant — pseudonymous, coarse location only (no coordinates), no real label
    let location_ref = match (
        opt_str(&source_meta, "/location/city"),
        opt_str(&source_meta, "/location/country"),
    ) {
        (Some(city), Some(country)) => Some(format!("{city}, {country}")),
        (Some(city), None) => Some(city),
        (None, Some(country)) => Some(country),
        (None, None) => None,
    };

    // period — one canonical record covers one interval
    let start = require_str(&record, "/timestamp", "canonical-record.json")?;
    let interval_minutes = record
        .pointer("/interval_minutes")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            Error::Bundle("missing `/interval_minutes` in canonical-record.json".into())
        })?;
    let start_dt = DateTime::parse_from_rfc3339(&start)
        .map_err(|_| Error::InvalidTimestamp(start.clone()))?
        .with_timezone(&Utc);
    let end = (start_dt + Duration::minutes(interval_minutes as i64))
        .to_rfc3339_opts(SecondsFormat::Secs, true);

    // summary — never includes raw rows; metrics are aggregate only
    let total_energy_kwh = record
        .pointer("/telemetry/energy_since_start_wh")
        .and_then(Value::as_i64)
        .map(|wh| (wh as f64) / 1000.0);
    let anomalies_count = read_json(&bundle_dir.join("events.json"))
        .ok()
        .and_then(|v| v.as_array().map(|a| a.len() as u64))
        .or_else(|| {
            record
                .pointer("/events")
                .and_then(Value::as_array)
                .map(|a| a.len() as u64)
        });

    // signature — over the canonical record bytes (not the v1 bundle)
    let algorithm =
        opt_str(&envelope, "/signature/algorithm").unwrap_or_else(|| "ed25519".to_string());
    let signature_value = require_str(
        &envelope,
        "/signature/signature_value",
        "signature-envelope.json",
    )?;

    // anchor — reflects LOCAL state only; defaults to dry-run (nothing submitted
    // by default). A real reference is included only if an anchor response exists.
    let anchor_response = read_json(&bundle_dir.join("anchor-response.json")).ok();
    let anchor = Some(build_anchor_block(
        &canonical_hash,
        anchor_response.as_ref(),
    ));

    // files — pointers within the bundle directory
    let validation_report = if bundle_dir.join("verification-report.json").exists() {
        Some("verification-report.json".to_string())
    } else {
        None
    };

    Ok(EvidenceBundleV1 {
        schema_version: AGENTOS_BUNDLE_SCHEMA.to_string(),
        agent: AgentBlock {
            agent_id,
            agent_version: agent_version.to_string(),
            public_key,
        },
        plant: PlantBlock {
            asset_ref: pseudonymous_asset_ref(&plant_id),
            asset_label: None,
            technology: "solar_pv".to_string(),
            location_ref,
        },
        period: PeriodBlock {
            start,
            end,
            interval_minutes,
        },
        summary: SummaryBlock {
            record_count: 1,
            total_energy_kwh,
            availability_pct: None,
            curtailment_kwh: None,
            anomalies_count,
        },
        hashes: HashesBlock {
            // For a single-record PV bundle the committed value is the canonical
            // record hash, so data_hash and commitment_hash coincide.
            data_hash: canonical_hash.clone(),
            commitment_hash: canonical_hash.clone(),
            canonicalization: CANONICALIZATION.to_string(),
        },
        signature: SignatureBlock {
            algorithm,
            signer: public_key_from_envelope(&envelope),
            signature: signature_value,
        },
        anchor,
        files: Some(FilesBlock {
            manifest: "manifest.json".to_string(),
            data_summary: Some("source-metadata.json".to_string()),
            validation_report,
            signed_payload: Some("canonical-record.json".to_string()),
        }),
    })
}

fn public_key_from_envelope(envelope: &Value) -> String {
    opt_str(envelope, "/signature/public_key_b64").unwrap_or_default()
}

fn build_anchor_block(canonical_hash: &str, anchor_response: Option<&Value>) -> AnchorBlock {
    let reference = anchor_response
        .and_then(|r| opt_str(r, "/reference"))
        .filter(|s| !s.is_empty());
    let submission_status = match &reference {
        Some(_) => anchor_response
            .and_then(|r| opt_str(r, "/status"))
            .or_else(|| Some("submitted".to_string())),
        None => Some("dry_run_only".to_string()),
    };
    AnchorBlock {
        // The agent does not use AgentOS's controlled modes; export the safe
        // default. Nothing here is submitted by AgentOS.
        mode: "dry_run".to_string(),
        network: None,
        anchor_hash: Some(canonical_hash.to_string()),
        anchor_reference: reference,
        submission_status,
        verification_status: Some("not_checked".to_string()),
    }
}

/// Serialise the bundle as pretty JSON.
pub fn to_pretty_json(bundle: &EvidenceBundleV1) -> Result<String> {
    Ok(serde_json::to_string_pretty(bundle)?)
}
