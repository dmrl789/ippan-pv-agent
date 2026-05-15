//! Local verification of an evidence bundle.

use crate::bundle::{
    read_anchor_request, read_anchor_response_raw, read_canonical_bytes, read_envelope,
    read_manifest, write_verification_report, CANONICAL_FILE, ANCHOR_REQ_FILE, EVENTS_FILE,
    SIGNATURE_FILE, SOURCE_META_FILE, VERIFY_REPORT_SCHEMA,
};
use crate::canonical::to_canonical_bytes;
use crate::hashing::sha256_prefixed_hex;
use crate::signing::verify_envelope;
use crate::{Error, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct VerificationReport {
    pub schema: String,
    pub record_id: String,
    pub plant_id: String,
    pub canonical_hash: String,
    pub canonical_reproducible: bool,
    pub canonical_hash_matches_manifest: bool,
    pub signature_valid: bool,
    pub manifest_files_intact: bool,
    pub anchor_request_matches: bool,
    pub anchor_response_matches: Option<bool>,
    pub anchor_status: Option<String>,
    pub anchor_reference: Option<String>,
    pub overall_pass: bool,
}

/// Verify all local invariants. Writes `verification-report.json` to the bundle.
pub fn verify_local(bundle_dir: &Path) -> Result<VerificationReport> {
    let manifest = read_manifest(bundle_dir)?;
    let canonical_bytes = read_canonical_bytes(bundle_dir)?;

    // 1. Canonical bytes hash matches manifest.
    let recomputed_hash = sha256_prefixed_hex(&canonical_bytes);
    let canonical_hash_matches_manifest = recomputed_hash == manifest.canonical_hash;

    // 2. Canonical reproducibility: re-parsing + re-canonicalizing yields
    //    identical bytes. (Catches the case where the file on disk has been
    //    re-formatted away from canonical form.)
    let canonical_reproducible = match serde_json::from_slice::<Value>(&canonical_bytes) {
        Ok(v) => match to_canonical_bytes(&v) {
            Ok(re) => re == canonical_bytes,
            Err(_) => false,
        },
        Err(_) => false,
    };

    // 3. Signature.
    let envelope = read_envelope(bundle_dir)?;
    let signature_valid = verify_envelope(&envelope, &canonical_bytes).is_ok()
        && envelope.canonical_hash == manifest.canonical_hash
        && envelope.record_id == manifest.record_id
        && envelope.plant_id == manifest.plant_id;

    // 4. Manifest file hashes.
    let mut manifest_files_intact = true;
    for f in &manifest.files {
        let p = bundle_dir.join(&f.path);
        match fs::read(&p) {
            Ok(bytes) => {
                if sha256_prefixed_hex(&bytes) != f.sha256 {
                    manifest_files_intact = false;
                }
            }
            Err(_) => manifest_files_intact = false,
        }
    }
    // Belt-and-braces: ensure key files are listed.
    for required in [
        CANONICAL_FILE,
        SIGNATURE_FILE,
        SOURCE_META_FILE,
        EVENTS_FILE,
        ANCHOR_REQ_FILE,
    ] {
        if !manifest.files.iter().any(|f| f.path == required) {
            manifest_files_intact = false;
        }
    }

    // 5. Anchor request commitment matches canonical hash.
    let anchor_req = read_anchor_request(bundle_dir)?;
    let anchor_request_matches =
        anchor_req.commitment.algorithm == "sha256"
            && anchor_req.commitment.hash == manifest.canonical_hash
            && anchor_req.evidence_bundle_id == manifest.record_id;

    // 6. Anchor response, if present and non-pending, should reference the
    //    same hash.
    let anchor_resp = read_anchor_response_raw(bundle_dir)?;
    let status = anchor_resp
        .get("status")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let reference = anchor_resp
        .get("reference")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let anchor_response_matches = match status.as_deref() {
        Some("pending") | None => None,
        Some(_) => {
            let anchor_hash = anchor_resp
                .get("anchor_hash")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            match anchor_hash {
                Some(h) => Some(h == manifest.canonical_hash || h == strip_prefix(&manifest.canonical_hash)),
                None => Some(true), // No anchor_hash echoed back; treat as not contradicting.
            }
        }
    };

    let mut overall_pass = canonical_hash_matches_manifest
        && canonical_reproducible
        && signature_valid
        && manifest_files_intact
        && anchor_request_matches;
    if let Some(false) = anchor_response_matches {
        overall_pass = false;
    }

    let report = VerificationReport {
        schema: VERIFY_REPORT_SCHEMA.into(),
        record_id: manifest.record_id.clone(),
        plant_id: manifest.plant_id.clone(),
        canonical_hash: manifest.canonical_hash.clone(),
        canonical_reproducible,
        canonical_hash_matches_manifest,
        signature_valid,
        manifest_files_intact,
        anchor_request_matches,
        anchor_response_matches,
        anchor_status: status,
        anchor_reference: reference,
        overall_pass,
    };

    let report_value = json!({
        "schema": report.schema,
        "record_id": report.record_id,
        "plant_id": report.plant_id,
        "canonical_hash": report.canonical_hash,
        "checks": {
            "canonical_reproducible": report.canonical_reproducible,
            "canonical_hash_matches_manifest": report.canonical_hash_matches_manifest,
            "signature_valid": report.signature_valid,
            "manifest_files_intact": report.manifest_files_intact,
            "anchor_request_matches": report.anchor_request_matches,
            "anchor_response_matches": report.anchor_response_matches,
        },
        "anchor_status": report.anchor_status,
        "anchor_reference": report.anchor_reference,
        "overall_pass": report.overall_pass,
    });
    write_verification_report(bundle_dir, &report_value)?;

    Ok(report)
}

fn strip_prefix(s: &str) -> String {
    s.strip_prefix("sha256:").unwrap_or(s).to_string()
}

/// Convenience: returns `Ok(())` if the bundle verifies, `Err(...)` otherwise.
pub fn require_pass(bundle_dir: &Path) -> Result<VerificationReport> {
    let r = verify_local(bundle_dir)?;
    if r.overall_pass {
        Ok(r)
    } else {
        Err(Error::Verification(format!(
            "verification failed for {}: hash={}/{} sig={} manifest={} anchor_req={} anchor_resp={:?}",
            r.record_id,
            r.canonical_hash_matches_manifest,
            r.canonical_reproducible,
            r.signature_valid,
            r.manifest_files_intact,
            r.anchor_request_matches,
            r.anchor_response_matches
        )))
    }
}
