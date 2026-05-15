mod common;

use ippan_pv_agent::anchor;
use ippan_pv_agent::bundle::{build_bundle, BuildOptions};
use ippan_pv_agent::config::IppanConfig;
use ippan_pv_agent::demo::{palermo_events, palermo_raw_input};
use ippan_pv_agent::signing::OperatorKey;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use tempfile::tempdir;

// Each test gets its own env var name so tests can run in parallel without
// racing on a shared global. Tests that don't need a token simply omit them.
const TOKEN_ENV_T1: &str = "IPPAN_TEST_TOKEN_T1";
const TOKEN_ENV_T5: &str = "IPPAN_TEST_TOKEN_T5";

/// Captured request state shared with the test thread.
#[derive(Default, Clone, Debug)]
struct Captured {
    method: String,
    path: String,
    authorization: Option<String>,
    body: String,
}

fn start_mock(response_body: String, captured_tx: Sender<Captured>) -> (u16, Arc<Mutex<bool>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    let stop_flag = Arc::new(Mutex::new(false));
    let stop_flag_c = stop_flag.clone();
    thread::spawn(move || {
        listener.set_nonblocking(false).ok();
        if let Ok((mut stream, _)) = listener.accept() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());

            let mut request_line = String::new();
            reader.read_line(&mut request_line).ok();
            let parts: Vec<&str> = request_line.split_whitespace().collect();
            let method = parts.first().copied().unwrap_or("").to_string();
            let path = parts.get(1).copied().unwrap_or("").to_string();

            let mut content_length: usize = 0;
            let mut authorization: Option<String> = None;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    let name_lc = name.trim().to_ascii_lowercase();
                    let value = value.trim_end_matches(['\r', '\n']).trim();
                    if name_lc == "content-length" {
                        content_length = value.parse().unwrap_or(0);
                    } else if name_lc == "authorization" {
                        authorization = Some(value.to_string());
                    }
                }
            }

            let mut body_buf = vec![0u8; content_length];
            if content_length > 0 {
                reader.read_exact(&mut body_buf).ok();
            }
            let body = String::from_utf8_lossy(&body_buf).into_owned();

            let _ = captured_tx.send(Captured {
                method,
                path,
                authorization,
                body,
            });

            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
            let mut g = stop_flag_c.lock().unwrap();
            *g = true;
        }
    });
    (port, stop_flag)
}

fn make_bundle() -> (tempfile::TempDir, std::path::PathBuf) {
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
    let bundle_dir = built.bundle_dir.clone();
    (dir, bundle_dir)
}

#[test]
fn submit_sends_commitment_only_with_bearer_header() {
    let (tx, rx) = channel();
    let response = serde_json::json!({
        "status": "submitted",
        "reference": "ippan-l1-anchor:abc",
        "anchor_hash": "deadbeef",
        "sequence": 1,
        "submitted_at_logical": 999
    })
    .to_string();
    let (port, _stop) = start_mock(response, tx);

    let (_tempdir, bundle) = make_bundle();
    std::env::set_var(TOKEN_ENV_T1, "super-secret-test-token");

    let ippan = IppanConfig {
        endpoint: format!("http://127.0.0.1:{}", port),
        anchor_path: "/v1/anchors".into(),
        admin_token_env: Some(TOKEN_ENV_T1.into()),
        submit_anchors: false,
    };

    let res = anchor::submit(&bundle, &ippan, true, false).expect("submit ok");
    assert_eq!(res.response_value["status"], "submitted");

    let captured = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("captured");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/v1/anchors");
    assert_eq!(
        captured.authorization.as_deref(),
        Some("Bearer super-secret-test-token")
    );

    // Body must contain commitment but NOT telemetry.
    let body_json: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
    assert!(body_json.get("commitment").is_some());
    assert_eq!(body_json["commitment"]["algorithm"], "sha256");
    assert!(body_json["commitment"]["hash"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    // No telemetry fields anywhere in the serialized request.
    let body_text = &captured.body;
    for forbidden in [
        "ghi_w_m2",
        "ambient_temperature",
        "dc_power",
        "ac_power",
        "meter_power",
        "performance_ratio",
        "energy_since_start",
    ] {
        assert!(
            !body_text.contains(forbidden),
            "anchor request body must not include telemetry field `{}`",
            forbidden
        );
    }

    std::env::remove_var(TOKEN_ENV_T1);
}

#[test]
fn submit_refused_without_token() {
    let (_tempdir, bundle) = make_bundle();
    // Use a dedicated env var name to avoid racing other tests.
    let env_name = "IPPAN_TEST_TOKEN_T2_DEFINITELY_UNSET";
    std::env::remove_var(env_name);

    let ippan = IppanConfig {
        endpoint: "http://127.0.0.1:1".into(),
        anchor_path: "/v1/anchors".into(),
        admin_token_env: Some(env_name.into()),
        submit_anchors: false,
    };
    let err = anchor::submit(&bundle, &ippan, true, false).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("admin token") || msg.contains(env_name));
}

#[test]
fn submit_refused_when_disabled_by_config_and_no_override() {
    let (_tempdir, bundle) = make_bundle();
    let ippan = IppanConfig {
        endpoint: "http://127.0.0.1:1".into(),
        anchor_path: "/v1/anchors".into(),
        admin_token_env: None,
        submit_anchors: false,
    };
    let err = anchor::submit(&bundle, &ippan, false, false).unwrap_err();
    assert!(err.to_string().contains("submit_anchors=false"));
}

#[test]
fn failed_anchor_keeps_local_evidence_intact() {
    // Nothing listens on this port — submission will fail at transport.
    let (_tempdir, bundle) = make_bundle();
    let before = std::fs::read(bundle.join("canonical-record.json")).unwrap();

    let ippan = IppanConfig {
        endpoint: "http://127.0.0.1:1".into(),
        anchor_path: "/v1/anchors".into(),
        admin_token_env: None,
        submit_anchors: false,
    };
    let _ = anchor::submit(&bundle, &ippan, true, false);

    let after = std::fs::read(bundle.join("canonical-record.json")).unwrap();
    assert_eq!(before, after, "canonical record must not be modified by failed anchor");
}

#[test]
fn second_submit_refused_unless_force() {
    let (tx, _rx) = channel();
    let response = serde_json::json!({
        "status": "submitted",
        "reference": "ippan-l1-anchor:abc",
        "anchor_hash": "deadbeef",
        "sequence": 1,
        "submitted_at_logical": 999
    })
    .to_string();
    let (port, _stop) = start_mock(response, tx);

    let (_tempdir, bundle) = make_bundle();
    std::env::set_var(TOKEN_ENV_T5, "tok");

    let ippan = IppanConfig {
        endpoint: format!("http://127.0.0.1:{}", port),
        anchor_path: "/v1/anchors".into(),
        admin_token_env: Some(TOKEN_ENV_T5.into()),
        submit_anchors: false,
    };

    anchor::submit(&bundle, &ippan, true, false).expect("first submit");
    let err = anchor::submit(&bundle, &ippan, true, false).unwrap_err();
    assert!(err.to_string().contains("already has a non-pending anchor response"));

    std::env::remove_var(TOKEN_ENV_T5);
}
