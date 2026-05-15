mod common;

use ippan_pv_agent::bundle::{build_bundle, read_envelope, BuildOptions};
use ippan_pv_agent::demo::{palermo_events, palermo_raw_input};
use ippan_pv_agent::signing::{verify_envelope, OperatorKey};
use tempfile::tempdir;

#[test]
fn signature_round_trips_against_canonical_bytes() {
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
    let env = read_envelope(&built.bundle_dir).unwrap();
    let bytes = std::fs::read(built.bundle_dir.join("canonical-record.json")).unwrap();
    assert!(verify_envelope(&env, &bytes).is_ok());
}

#[test]
fn modified_payload_fails_verification() {
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
    let env = read_envelope(&built.bundle_dir).unwrap();
    let tampered = b"{\"x\":1}";
    assert!(verify_envelope(&env, tampered).is_err());
}

#[test]
fn signature_envelope_stores_key_ref_not_private_seed() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:my-test-plant");
    let built = build_bundle(
        &cfg,
        &palermo_raw_input(),
        &palermo_events(),
        &key,
        &BuildOptions::default(),
    )
    .unwrap();
    let env_bytes = std::fs::read(built.bundle_dir.join("signature-envelope.json")).unwrap();
    let env_text = std::str::from_utf8(&env_bytes).unwrap();
    assert!(env_text.contains("key:my-test-plant"));
    // The seed/secret must never appear here.
    assert!(!env_text.contains("secret_seed_b64"));
    assert!(!env_text.contains("BEGIN PRIVATE KEY"));
}

#[test]
fn cannot_verify_with_a_different_public_key() {
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

    let mut env = read_envelope(&built.bundle_dir).unwrap();
    // Substitute the public key for an unrelated one.
    let other = OperatorKey::generate_demo("key:other");
    env.signature.public_key_b64 = other.public_key_b64();
    let bytes = std::fs::read(built.bundle_dir.join("canonical-record.json")).unwrap();
    assert!(verify_envelope(&env, &bytes).is_err());
}
