use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn pv_agent() -> Command {
    Command::cargo_bin("pv-agent").expect("binary built")
}

#[test]
fn demo_creates_a_bundle_that_verifies() {
    let dir = tempdir().unwrap();
    let base = dir.path().join("data/pv-agent");

    pv_agent()
        .args([
            "demo",
            "--plant",
            "palermo-1mw",
            "--base-dir",
        ])
        .arg(&base)
        .assert()
        .success()
        .stdout(predicate::str::contains("Canonical record created: YES"))
        .stdout(predicate::str::contains("Evidence bundle saved: YES"))
        .stdout(predicate::str::contains("IPPAN L1 anchor submitted: NO"));

    let bundle = base.join("palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z");
    assert!(bundle.exists(), "bundle directory should exist at {}", bundle.display());

    pv_agent()
        .args(["verify", "--bundle"])
        .arg(&bundle)
        .assert()
        .success()
        .stdout(predicate::str::contains("PV evidence verification: PASS"));
}

#[test]
fn inspect_does_not_leak_secrets() {
    let dir = tempdir().unwrap();
    let base = dir.path().join("data/pv-agent");
    pv_agent()
        .args(["demo", "--plant", "palermo-1mw", "--base-dir"])
        .arg(&base)
        .assert()
        .success();
    let bundle = base.join("palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z");

    let out = pv_agent()
        .args(["inspect", "--bundle"])
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);

    // Visible fields
    assert!(text.contains("Plant ID:"));
    assert!(text.contains("Canonical hash:"));
    assert!(text.contains("Telemetry"));

    // Forbidden disclosures
    for forbidden in [
        "secret_seed_b64",
        "BEGIN PRIVATE KEY",
        "Bearer ",
        "IPPAN_ADMIN_TOKEN=",
    ] {
        assert!(!text.contains(forbidden), "inspect output leaked `{}`", forbidden);
    }
}

#[test]
fn run_once_with_example_files_succeeds() {
    let dir = tempdir().unwrap();
    let base = dir.path().join("data/pv-agent");
    std::fs::create_dir_all(base.join("keys")).unwrap();

    // Generate a demo key under the storage dir.
    let key_path = base.join("keys/demo-key.json");
    pv_agent()
        .args(["generate-demo-key", "--out"])
        .arg(&key_path)
        .args(["--key-ref", "key:plant-palermo-001"])
        .assert()
        .success();

    // Write a config pointing at this storage + key.
    let cfg_path = dir.path().join("pv-agent.toml");
    let cfg_text = format!(
        r#"
[agent]
agent_id = "pv-agent-palermo-001"
agent_type = "pv_plant_agent"
plant_id = "palermo-pv-001"
operator_key_ref = "key:plant-palermo-001"
production_mode = false

[storage]
base_dir = "{}"

[ippan]
endpoint = "http://127.0.0.1:18181"
anchor_path = "/v1/anchors"
admin_token_env = "IPPAN_ADMIN_TOKEN"
submit_anchors = false

[events]
lookback_minutes = 240

[key]
key_file = "{}"
"#,
        base.to_string_lossy().replace('\\', "\\\\"),
        key_path.to_string_lossy().replace('\\', "\\\\"),
    );
    std::fs::write(&cfg_path, cfg_text).unwrap();

    // Locate the example files relative to the workspace root.
    let workspace = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let input = std::path::PathBuf::from(&workspace).join("examples/pv/palermo-telemetry.json");
    let events = std::path::PathBuf::from(&workspace).join("examples/pv/palermo-events.json");

    pv_agent()
        .args(["run-once", "--input"])
        .arg(&input)
        .args(["--events"])
        .arg(&events)
        .args(["--config"])
        .arg(&cfg_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Record ID:"))
        .stdout(predicate::str::contains("Canonical hash:"));

    let bundle = base.join("palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z");
    pv_agent()
        .args(["verify", "--bundle"])
        .arg(&bundle)
        .assert()
        .success()
        .stdout(predicate::str::contains("PV evidence verification: PASS"));
}

#[test]
fn verify_fails_after_canonical_record_is_tampered() {
    let dir = tempdir().unwrap();
    let base = dir.path().join("data/pv-agent");
    pv_agent()
        .args(["demo", "--plant", "palermo-1mw", "--base-dir"])
        .arg(&base)
        .assert()
        .success();
    let bundle = base.join("palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z");
    let canonical = bundle.join("canonical-record.json");
    let mut bytes = std::fs::read(&canonical).unwrap();
    bytes.push(b' ');
    std::fs::write(&canonical, &bytes).unwrap();

    pv_agent()
        .args(["verify", "--bundle"])
        .arg(&bundle)
        .assert()
        .failure()
        .stdout(predicate::str::contains("PV evidence verification: FAIL"));
}

#[test]
fn anchor_status_without_reference_errors_cleanly() {
    let dir = tempdir().unwrap();
    let base = dir.path().join("data/pv-agent");
    pv_agent()
        .args(["demo", "--plant", "palermo-1mw", "--base-dir"])
        .arg(&base)
        .assert()
        .success();
    let bundle = base.join("palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z");

    let cfg = dir.path().join("pv-agent.toml");
    std::fs::write(
        &cfg,
        r#"
[agent]
agent_id = "a"
agent_type = "pv_plant_agent"
plant_id = "palermo-pv-001"
operator_key_ref = "key:plant-palermo-001"

[storage]
base_dir = "data/pv-agent"

[ippan]
endpoint = "http://127.0.0.1:1"
"#,
    )
    .unwrap();

    pv_agent()
        .args(["anchor-status", "--bundle"])
        .arg(&bundle)
        .args(["--config"])
        .arg(&cfg)
        .assert()
        .failure();
}
