mod common;

use ippan_pv_agent::agentos_bundle::{
    build_agentos_bundle, pseudonymous_asset_ref, to_pretty_json, EvidenceBundleV1,
    AGENTOS_BUNDLE_SCHEMA,
};
use ippan_pv_agent::bundle::{build_bundle, BuildOptions};
use ippan_pv_agent::demo::{palermo_events, palermo_raw_input};
use ippan_pv_agent::signing::OperatorKey;
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
