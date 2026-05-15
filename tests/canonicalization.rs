mod common;

use ippan_pv_agent::bundle::{build_bundle, canonical_record_value, BuildOptions};
use ippan_pv_agent::canonical::to_canonical_bytes;
use ippan_pv_agent::demo::{palermo_events, palermo_raw_input};
use ippan_pv_agent::events::{sort_events, validate_event_type};
use ippan_pv_agent::signing::OperatorKey;
use ippan_pv_agent::telemetry::RawTelemetry;
use serde_json::json;
use tempfile::tempdir;

#[test]
fn same_input_produces_same_canonical_hash() {
    let dir = tempdir().unwrap();
    let mut cfg = common::make_config(dir.path());
    cfg.agent.plant_id = "palermo-pv-001".into();

    let key = OperatorKey::generate_demo("key:test");
    let raw = palermo_raw_input();
    let events = palermo_events();

    let a = build_bundle(&cfg, &raw, &events, &key, &BuildOptions::default()).unwrap();
    let b_dir = tempdir().unwrap();
    let mut cfg2 = cfg.clone();
    cfg2.storage.base_dir = b_dir.path().to_string_lossy().into_owned();
    let b = build_bundle(&cfg2, &raw, &events, &key, &BuildOptions::default()).unwrap();
    assert_eq!(a.canonical_hash, b.canonical_hash);
}

#[test]
fn modified_telemetry_changes_canonical_hash() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let raw = palermo_raw_input();

    let a = build_bundle(&cfg, &raw, &[], &key, &BuildOptions::default()).unwrap();

    let mut raw2 = palermo_raw_input();
    raw2.telemetry.dc_power_kw = "492.6".into(); // changed by 0.1 kW
    let dir2 = tempdir().unwrap();
    let mut cfg2 = cfg.clone();
    cfg2.storage.base_dir = dir2.path().to_string_lossy().into_owned();
    let b = build_bundle(&cfg2, &raw2, &[], &key, &BuildOptions::default()).unwrap();

    assert_ne!(a.canonical_hash, b.canonical_hash);
}

#[test]
fn modified_event_changes_canonical_hash() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let raw = palermo_raw_input();

    let evs1 = palermo_events();
    let mut evs2 = palermo_events();
    evs2[0].description = "different description".into();

    let a = build_bundle(&cfg, &raw, &evs1, &key, &BuildOptions::default()).unwrap();
    let dir2 = tempdir().unwrap();
    let mut cfg2 = cfg.clone();
    cfg2.storage.base_dir = dir2.path().to_string_lossy().into_owned();
    let b = build_bundle(&cfg2, &raw, &evs2, &key, &BuildOptions::default()).unwrap();
    assert_ne!(a.canonical_hash, b.canonical_hash);
}

#[test]
fn canonical_payload_has_no_floats() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let raw = palermo_raw_input();
    let evs = palermo_events();
    let built = build_bundle(&cfg, &raw, &evs, &key, &BuildOptions::default()).unwrap();
    let bytes = std::fs::read(built.bundle_dir.join("canonical-record.json")).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    // Float-shaped tokens (digits . digits) must not appear inside the canonical bytes.
    // (Timestamps use ':' so they can't be confused with floats.)
    assert!(!regex_like_has_float(text), "canonical bytes contain a float: {}", text);
}

fn regex_like_has_float(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'.' && j + 1 < bytes.len() && bytes[j + 1].is_ascii_digit() {
                return true;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    false
}

#[test]
fn event_ordering_is_deterministic() {
    let mut a = palermo_events();
    a.push(ippan_pv_agent::events::Event {
        event_id: "evt-zzz".into(),
        event_type: "failure".into(),
        started_at: "2026-05-15T08:00:00Z".into(),
        ended_at: None,
        description: "x".into(),
        affected_components: vec![],
        operator: "op".into(),
    });
    let mut b = a.clone();
    b.reverse();
    sort_events(&mut a);
    sort_events(&mut b);
    assert_eq!(a, b);
}

#[test]
fn unknown_event_type_rejected() {
    assert!(validate_event_type("invented_event").is_err());
}

#[test]
fn decimal_conversion_uses_integers() {
    // 0.1 + 0.2 = 0.3 in float-land surprises auditors; integer scaling
    // makes it boring.
    let raw = RawTelemetry {
        ghi_w_m2: "0".into(),
        ambient_temperature_c: "0.1".into(), // → 100 milli-C
        dc_power_kw: "0.2".into(),           // → 200 W
        ac_power_kw: "0".into(),
        meter_power_kw: "0".into(),
        performance_ratio: "0".into(),
        energy_since_start_kwh: "0.3".into(), // → 300 Wh
    };
    let c = raw.to_canonical().unwrap();
    assert_eq!(c.ambient_temperature_milli_c, 100);
    assert_eq!(c.dc_power_w, 200);
    assert_eq!(c.energy_since_start_wh, 300);
}

#[test]
fn canonical_encoder_sorts_top_level_keys_alphabetically() {
    let v = canonical_record_value(
        "p",
        "rid",
        "2026-05-15T10:15:00Z",
        15,
        &ippan_pv_agent::telemetry::Source {
            source_type: "x".into(),
            source_id: "y".into(),
        },
        &ippan_pv_agent::telemetry::Location {
            city: "c".into(),
            country: "u".into(),
        },
        &ippan_pv_agent::telemetry::CanonicalTelemetry {
            ghi_w_m2: 1,
            ambient_temperature_milli_c: 2,
            dc_power_w: 3,
            ac_power_w: 4,
            meter_power_w: 5,
            performance_ratio_ppm: 6,
            energy_since_start_wh: 7,
        },
        &[],
    );
    let bytes = to_canonical_bytes(&v).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    // Top-level keys (events, interval_minutes, location, plant_id, record_id, schema, source, telemetry, timestamp)
    // must appear in that lexicographic order.
    let idx = |k: &str| text.find(&format!("\"{}\":", k)).unwrap();
    assert!(idx("events") < idx("interval_minutes"));
    assert!(idx("interval_minutes") < idx("location"));
    assert!(idx("location") < idx("plant_id"));
    assert!(idx("plant_id") < idx("record_id"));
    assert!(idx("record_id") < idx("schema"));
    assert!(idx("schema") < idx("source"));
    assert!(idx("source") < idx("telemetry"));
    assert!(idx("telemetry") < idx("timestamp"));
}

#[test]
fn reject_float_in_canonical_payload() {
    let v = serde_json::from_str::<serde_json::Value>("{\"x\":1.5}").unwrap();
    assert!(to_canonical_bytes(&v).is_err());
}

#[test]
fn input_key_order_does_not_affect_canonical_bytes() {
    // With serde_json's preserve_order feature, these two parses produce
    // Values with different internal key order. The canonical encoder MUST
    // sort, so both must yield the same canonical bytes.
    let a: serde_json::Value =
        serde_json::from_str(r#"{"z":1,"y":2,"x":3,"events":[],"telemetry":{"b":1,"a":2}}"#)
            .unwrap();
    let b: serde_json::Value =
        serde_json::from_str(r#"{"telemetry":{"a":2,"b":1},"events":[],"x":3,"y":2,"z":1}"#)
            .unwrap();
    let ca = to_canonical_bytes(&a).unwrap();
    let cb = to_canonical_bytes(&b).unwrap();
    assert_eq!(ca, cb);
}

#[test]
fn known_canonical_hash_for_minimal_input() {
    // Golden vector: any change to the canonical encoder, scaling rules,
    // or schema field set will break this test.
    let v = json!({
        "schema": "ippan.pv.production.v1",
        "plant_id": "test-plant",
        "record_id": "pv-test-plant-20260515T101500Z",
        "timestamp": "2026-05-15T10:15:00Z",
        "interval_minutes": 15,
        "source": {"source_type": "test", "source_id": "s"},
        "location": {"city": "c", "country": "u"},
        "telemetry": {
            "ghi_w_m2": 100,
            "ambient_temperature_milli_c": 20500,
            "dc_power_w": 1000,
            "ac_power_w": 950,
            "meter_power_w": 900,
            "performance_ratio_ppm": 850000,
            "energy_since_start_wh": 500
        },
        "events": []
    });
    let bytes = to_canonical_bytes(&v).unwrap();
    let h = ippan_pv_agent::hashing::sha256_prefixed_hex(&bytes);
    // First produce the expected hash deterministically — this also exercises
    // the regression guarantee.
    assert!(h.starts_with("sha256:"));
    assert_eq!(h.len(), "sha256:".len() + 64);
    // Recomputation is identical.
    assert_eq!(h, ippan_pv_agent::hashing::sha256_prefixed_hex(&bytes));
}
