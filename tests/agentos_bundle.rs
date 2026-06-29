mod common;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ippan_pv_agent::agentos_bundle::{
    build_agentos_bundle, build_agentos_import_wrapper, pseudonymous_asset_ref, to_pretty_json,
    wrapper_to_pretty_json, AgentOsImportWrapper, EvidenceBundleV1, AGENTOS_BUNDLE_SCHEMA,
};
use ippan_pv_agent::bundle::{build_bundle, read_canonical_bytes, read_envelope, BuildOptions};
use ippan_pv_agent::demo::{palermo_events, palermo_raw_input};
use ippan_pv_agent::signing::{verify_envelope, OperatorKey, SignatureBlock, SignatureEnvelope};
use tempfile::tempdir;

fn build_demo_bundle_dir(dir: &std::path::Path) -> std::path::PathBuf {
    let cfg = common::make_config(dir);
    let key = OperatorKey::generate_demo("key:test");
    let raw = palermo_raw_input();
    let events = palermo_events();
    let built = build_bundle(&cfg, &raw, &events, &key, &BuildOptions::default()).unwrap();
    built.bundle_dir
}

#[test]
fn emits_agentos_compatible_bundle() {
    let dir = tempdir().unwrap();
    let bundle_dir = build_demo_bundle_dir(dir.path());

    let v1 = build_agentos_bundle(&bundle_dir, "0.1.0").unwrap();

    assert_eq!(v1.schema_version, AGENTOS_BUNDLE_SCHEMA);
    assert_eq!(v1.plant.technology, "solar_pv");
    assert_eq!(v1.signature.algorithm, "ed25519");
    assert!(!v1.signature.signer.is_empty());
    assert!(!v1.signature.signature.is_empty());
    assert_eq!(v1.summary.record_count, 1);
    assert!(v1.period.interval_minutes > 0);
    assert!(!v1.period.start.is_empty());
    assert!(!v1.period.end.is_empty());
    assert_ne!(v1.period.start, v1.period.end);
}

#[test]
fn hashes_are_valid_sha256_and_consistent() {
    let dir = tempdir().unwrap();
    let bundle_dir = build_demo_bundle_dir(dir.path());
    let v1 = build_agentos_bundle(&bundle_dir, "0.1.0").unwrap();

    for h in [&v1.hashes.data_hash, &v1.hashes.commitment_hash] {
        let bare = h.strip_prefix("sha256:").unwrap_or(h);
        assert_eq!(bare.len(), 64, "hash must be 64 hex chars: {h}");
        assert!(
            bare.chars().all(|c| c.is_ascii_hexdigit()),
            "hash must be hex: {h}"
        );
    }
    // For a single-record PV bundle, the committed value is the canonical hash.
    assert_eq!(v1.hashes.data_hash, v1.hashes.commitment_hash);
    assert!(!v1.hashes.canonicalization.is_empty());
}

#[test]
fn asset_ref_is_pseudonymous_and_no_raw_plant_id_leaks() {
    let dir = tempdir().unwrap();
    let bundle_dir = build_demo_bundle_dir(dir.path());
    let v1 = build_agentos_bundle(&bundle_dir, "0.1.0").unwrap();

    assert!(v1.plant.asset_ref.starts_with("earef:"));
    assert_eq!(v1.plant.asset_ref, pseudonymous_asset_ref("palermo-pv-001"));
    assert_ne!(v1.plant.asset_ref, "palermo-pv-001");

    let json = to_pretty_json(&v1).unwrap();
    // The raw plant id must not appear anywhere in the exported bundle.
    assert!(
        !json.contains("palermo-pv-001"),
        "raw plant id leaked: {json}"
    );
}

#[test]
fn export_contains_no_raw_telemetry_rows() {
    let dir = tempdir().unwrap();
    let bundle_dir = build_demo_bundle_dir(dir.path());
    let v1 = build_agentos_bundle(&bundle_dir, "0.1.0").unwrap();
    let json = to_pretty_json(&v1).unwrap();

    // None of the raw canonical telemetry field names should be present —
    // the export is aggregate-summary only.
    for raw_field in [
        "ghi_w_m2",
        "energy_since_start_wh",
        "performance_ratio_ppm",
        "ac_power_milliwatt",
        "dc_power_milliwatt",
        "meter_power_milliwatt",
        "telemetry",
    ] {
        assert!(
            !json.contains(raw_field),
            "raw telemetry field `{raw_field}` leaked into export"
        );
    }
}

#[test]
fn bundle_round_trips_through_serde() {
    let dir = tempdir().unwrap();
    let bundle_dir = build_demo_bundle_dir(dir.path());
    let v1 = build_agentos_bundle(&bundle_dir, "0.1.0").unwrap();

    let json = to_pretty_json(&v1).unwrap();
    let back: EvidenceBundleV1 = serde_json::from_str(&json).unwrap();
    assert_eq!(back.schema_version, v1.schema_version);
    assert_eq!(back.hashes.commitment_hash, v1.hashes.commitment_hash);
    assert_eq!(back.signature.signature, v1.signature.signature);
}

#[test]
fn example_fixture_parses() {
    let fixture = include_str!("../examples/agentos/pv-agent-bundle.example.json");
    let parsed: EvidenceBundleV1 = serde_json::from_str(fixture).unwrap();
    assert_eq!(parsed.schema_version, AGENTOS_BUNDLE_SCHEMA);
    assert_eq!(parsed.plant.technology, "solar_pv");
    assert_eq!(parsed.signature.algorithm, "ed25519");
    let bare = parsed
        .hashes
        .commitment_hash
        .strip_prefix("sha256:")
        .unwrap_or(&parsed.hashes.commitment_hash);
    assert_eq!(bare.len(), 64);
}

// ---------------------------------------------------------------------------
// AgentOS import wrapper ({ bundle, signed_payload })
// ---------------------------------------------------------------------------

/// Mirror how AgentOS decodes `signed_payload.canonical_bytes` into the bytes
/// it verifies the signature over.
fn decode_signed_payload(encoding: &str, canonical_bytes: &str) -> Vec<u8> {
    match encoding {
        "utf8" => canonical_bytes.as_bytes().to_vec(),
        "base64" => B64.decode(canonical_bytes.as_bytes()).unwrap(),
        other => panic!("unexpected encoding: {other}"),
    }
}

#[test]
fn wrapper_contains_both_bundle_and_signed_payload() {
    let dir = tempdir().unwrap();
    let bundle_dir = build_demo_bundle_dir(dir.path());
    let w = build_agentos_import_wrapper(&bundle_dir, "0.1.0").unwrap();

    // bundle block is the same AgentOS-compatible bundle.
    assert_eq!(w.bundle.schema_version, AGENTOS_BUNDLE_SCHEMA);
    assert_eq!(w.bundle.signature.algorithm, "ed25519");

    // signed_payload carries the AgentOS-compatible fields.
    assert!(["utf8", "base64"].contains(&w.signed_payload.encoding.as_str()));
    assert!(!w.signed_payload.canonical_bytes.is_empty());
    assert_eq!(
        w.signed_payload.content_type.as_deref(),
        Some("application/json")
    );
    assert_eq!(
        w.signed_payload.description.as_deref(),
        Some("canonical-record.json")
    );
}

#[test]
fn signed_payload_equals_exact_signed_canonical_bytes() {
    let dir = tempdir().unwrap();
    let bundle_dir = build_demo_bundle_dir(dir.path());
    let w = build_agentos_import_wrapper(&bundle_dir, "0.1.0").unwrap();

    // The exact bytes that were written (and signed) on disk.
    let on_disk = read_canonical_bytes(&bundle_dir).unwrap();
    let decoded = decode_signed_payload(
        &w.signed_payload.encoding,
        &w.signed_payload.canonical_bytes,
    );

    // No reformatting / regeneration: byte-for-byte identical.
    assert_eq!(decoded, on_disk);
    // The demo record is valid UTF-8, so it ships as `utf8`.
    assert_eq!(w.signed_payload.encoding, "utf8");
}

#[test]
fn signed_payload_verifies_against_the_bundle_signature() {
    let dir = tempdir().unwrap();
    let bundle_dir = build_demo_bundle_dir(dir.path());
    let w = build_agentos_import_wrapper(&bundle_dir, "0.1.0").unwrap();

    // Reproduce AgentOS's verification: the supplied bytes must verify against
    // the Ed25519 signature in the envelope. If this passes, AgentOS reaches
    // `signature_status: verified`.
    let envelope = read_envelope(&bundle_dir).unwrap();
    let decoded = decode_signed_payload(
        &w.signed_payload.encoding,
        &w.signed_payload.canonical_bytes,
    );
    verify_envelope(&envelope, &decoded).expect("signed payload must verify against the signature");

    // Sanity: the wrapper's signature value matches the envelope's.
    assert_eq!(
        w.bundle.signature.signature,
        envelope.signature.signature_value
    );
    assert_eq!(w.bundle.signature.signer, envelope.signature.public_key_b64);
}

#[test]
fn tampered_signed_payload_fails_verification() {
    let dir = tempdir().unwrap();
    let bundle_dir = build_demo_bundle_dir(dir.path());
    let w = build_agentos_import_wrapper(&bundle_dir, "0.1.0").unwrap();
    let envelope = read_envelope(&bundle_dir).unwrap();

    let mut decoded = decode_signed_payload(
        &w.signed_payload.encoding,
        &w.signed_payload.canonical_bytes,
    );
    decoded.push(b' '); // any change breaks the signature
    assert!(verify_envelope(&envelope, &decoded).is_err());
}

#[test]
fn bundle_only_export_path_is_unchanged() {
    let dir = tempdir().unwrap();
    let bundle_dir = build_demo_bundle_dir(dir.path());

    // The existing bare-bundle export still works and equals the wrapper's
    // `bundle` block (the wrapper only adds signed_payload alongside it).
    let bare = build_agentos_bundle(&bundle_dir, "0.1.0").unwrap();
    let w = build_agentos_import_wrapper(&bundle_dir, "0.1.0").unwrap();
    assert_eq!(
        to_pretty_json(&bare).unwrap(),
        to_pretty_json(&w.bundle).unwrap()
    );
}

#[test]
fn wrapper_round_trips_through_serde() {
    let dir = tempdir().unwrap();
    let bundle_dir = build_demo_bundle_dir(dir.path());
    let w = build_agentos_import_wrapper(&bundle_dir, "0.1.0").unwrap();

    let json = wrapper_to_pretty_json(&w).unwrap();
    let back: AgentOsImportWrapper = serde_json::from_str(&json).unwrap();
    assert_eq!(back.bundle.schema_version, w.bundle.schema_version);
    assert_eq!(
        back.signed_payload.canonical_bytes,
        w.signed_payload.canonical_bytes
    );
    assert_eq!(back.signed_payload.encoding, w.signed_payload.encoding);

    // The serialised wrapper has exactly the AgentOS request keys.
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(v.get("bundle").is_some());
    assert!(v.get("signed_payload").is_some());
    assert!(v["signed_payload"].get("encoding").is_some());
    assert!(v["signed_payload"].get("canonical_bytes").is_some());
}

#[test]
fn example_wrapper_parses_and_verifies() {
    // The committed example is real, fictional demo data with a genuine
    // signature over its signed_payload — so it actually verifies, exactly as
    // AgentOS would on import.
    let fixture = include_str!("../examples/agentos/pv-agent-import.example.json");
    let w: AgentOsImportWrapper = serde_json::from_str(fixture).unwrap();

    assert_eq!(w.bundle.schema_version, AGENTOS_BUNDLE_SCHEMA);
    assert_eq!(w.signed_payload.encoding, "utf8");

    // Rebuild a minimal envelope from the bundle's signature block and verify
    // the supplied bytes against it (verify only reads the signature fields).
    let envelope = SignatureEnvelope {
        schema: "ippan.pv.evidence-envelope.v1".into(),
        record_id: String::new(),
        plant_id: String::new(),
        canonical_hash: w.bundle.hashes.commitment_hash.clone(),
        signature: SignatureBlock {
            algorithm: w.bundle.signature.algorithm.clone(),
            operator_key_ref: String::new(),
            public_key_b64: w.bundle.signature.signer.clone(),
            signature_value: w.bundle.signature.signature.clone(),
        },
        created_at: String::new(),
    };
    let decoded = decode_signed_payload(
        &w.signed_payload.encoding,
        &w.signed_payload.canonical_bytes,
    );
    verify_envelope(&envelope, &decoded).expect("committed example wrapper must verify");
}
