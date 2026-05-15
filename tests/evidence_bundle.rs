mod common;

use ippan_pv_agent::bundle::{
    build_bundle, read_manifest, BuildOptions, ANCHOR_REQ_FILE, CANONICAL_FILE, EVENTS_FILE,
    MANIFEST_FILE, SIGNATURE_FILE, SOURCE_META_FILE,
};
use ippan_pv_agent::demo::{palermo_events, palermo_raw_input};
use ippan_pv_agent::signing::OperatorKey;
use ippan_pv_agent::verify::verify_local;
use std::fs;
use tempfile::tempdir;

#[test]
fn bundle_creates_all_required_files() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let built = build_bundle(
        &cfg,
        &palermo_raw_input(),
        &palermo_events(),
        &key,
        &BuildOptions::default(),
    )
    .unwrap();

    for f in [
        CANONICAL_FILE,
        MANIFEST_FILE,
        SIGNATURE_FILE,
        SOURCE_META_FILE,
        EVENTS_FILE,
        ANCHOR_REQ_FILE,
    ] {
        let p = built.bundle_dir.join(f);
        assert!(p.exists(), "{} should exist", f);
    }

    let manifest = read_manifest(&built.bundle_dir).unwrap();
    let listed: Vec<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();
    for required in [
        CANONICAL_FILE,
        SIGNATURE_FILE,
        SOURCE_META_FILE,
        EVENTS_FILE,
        ANCHOR_REQ_FILE,
    ] {
        assert!(listed.contains(&required), "manifest must list {}", required);
    }
}

#[test]
fn verification_passes_on_fresh_bundle() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let built = build_bundle(
        &cfg,
        &palermo_raw_input(),
        &palermo_events(),
        &key,
        &BuildOptions::default(),
    )
    .unwrap();
    let report = verify_local(&built.bundle_dir).unwrap();
    assert!(report.overall_pass, "report should pass: {:?}", report);
}

#[test]
fn verification_fails_when_canonical_record_modified() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let built = build_bundle(
        &cfg,
        &palermo_raw_input(),
        &palermo_events(),
        &key,
        &BuildOptions::default(),
    )
    .unwrap();

    // Append a benign space to the canonical bytes — invalidates the hash.
    let p = built.bundle_dir.join(CANONICAL_FILE);
    let mut bytes = fs::read(&p).unwrap();
    bytes.push(b' ');
    fs::write(&p, &bytes).unwrap();

    let report = verify_local(&built.bundle_dir).unwrap();
    assert!(!report.overall_pass);
}

#[test]
fn verification_fails_when_signature_envelope_modified() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let built = build_bundle(
        &cfg,
        &palermo_raw_input(),
        &palermo_events(),
        &key,
        &BuildOptions::default(),
    )
    .unwrap();

    let p = built.bundle_dir.join(SIGNATURE_FILE);
    let mut env: serde_json::Value =
        serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
    // Flip a single base64 char in the signature value.
    let sig = env["signature"]["signature_value"].as_str().unwrap().to_string();
    let flipped: String = sig
        .chars()
        .enumerate()
        .map(|(i, c)| if i == 0 { if c == 'A' { 'B' } else { 'A' } } else { c })
        .collect();
    env["signature"]["signature_value"] = serde_json::Value::String(flipped);
    fs::write(&p, serde_json::to_vec_pretty(&env).unwrap()).unwrap();

    let report = verify_local(&built.bundle_dir).unwrap();
    assert!(!report.overall_pass);
}

#[test]
fn verification_fails_when_manifest_hash_lies() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let built = build_bundle(
        &cfg,
        &palermo_raw_input(),
        &palermo_events(),
        &key,
        &BuildOptions::default(),
    )
    .unwrap();

    let p = built.bundle_dir.join(MANIFEST_FILE);
    let mut m: serde_json::Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
    // Corrupt the recorded canonical-record file hash.
    let files = m["files"].as_array_mut().unwrap();
    for f in files.iter_mut() {
        if f["path"] == "canonical-record.json" {
            f["sha256"] = serde_json::Value::String("sha256:0000000000000000000000000000000000000000000000000000000000000000".into());
        }
    }
    fs::write(&p, serde_json::to_vec_pretty(&m).unwrap()).unwrap();

    let report = verify_local(&built.bundle_dir).unwrap();
    assert!(!report.overall_pass);
}

#[test]
fn bundle_refuses_overwrite_without_force() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let raw = palermo_raw_input();
    let events = palermo_events();
    build_bundle(&cfg, &raw, &events, &key, &BuildOptions::default()).unwrap();
    let err = build_bundle(&cfg, &raw, &events, &key, &BuildOptions::default()).unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn bundle_allows_overwrite_with_force() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let raw = palermo_raw_input();
    let events = palermo_events();
    build_bundle(&cfg, &raw, &events, &key, &BuildOptions::default()).unwrap();
    let opts = BuildOptions {
        force: true,
        ..BuildOptions::default()
    };
    build_bundle(&cfg, &raw, &events, &key, &opts).unwrap();
}

#[test]
fn bundle_contains_no_secret_strings() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let built = build_bundle(
        &cfg,
        &palermo_raw_input(),
        &palermo_events(),
        &key,
        &BuildOptions::default(),
    )
    .unwrap();

    // No private-key markers, no bearer-token strings, no env var strings.
    for f in [
        CANONICAL_FILE,
        SIGNATURE_FILE,
        SOURCE_META_FILE,
        EVENTS_FILE,
        ANCHOR_REQ_FILE,
        MANIFEST_FILE,
    ] {
        let text = fs::read_to_string(built.bundle_dir.join(f)).unwrap();
        for forbidden in [
            "BEGIN PRIVATE KEY",
            "IPPAN_ADMIN_TOKEN=",
            "Bearer ",
            "secret_seed_b64",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} contained forbidden token `{}`",
                f,
                forbidden
            );
        }
    }
}
