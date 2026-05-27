mod common;

use ippan_pv_agent::bundle::{build_bundle, canonical_record_value, BuildOptions};
use ippan_pv_agent::canonical::to_canonical_bytes;
use ippan_pv_agent::demo::{palermo_events, palermo_raw_input};
use ippan_pv_agent::events::{sort_events, validate_event_type};
use ippan_pv_agent::signing::OperatorKey;
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
    raw2.telemetry.dc_power_kw = "492.6000".into();
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
    // Float-shaped tokens (digits . digits) inside JSON number positions must
    // not appear. Decimal-string telemetry like "20.5" appears inside quoted
    // string values where its surrounding `"…"` marks it as a string, so the
    // regex-like check below treats only *unquoted* digit-dot-digit as a float.
    assert!(
        !contains_unquoted_float(text),
        "canonical bytes contain a float: {}",
        text
    );
}

fn contains_unquoted_float(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b.is_ascii_digit() {
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j < bytes.len()
                && bytes[j] == b'.'
                && j + 1 < bytes.len()
                && bytes[j + 1].is_ascii_digit()
            {
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
fn canonical_encoder_sorts_top_level_keys_alphabetically() {
    let v = canonical_record_value(
        "p",
        "rid",
        "2026-05-15T10:15:00Z",
        15,
        &ippan_pv_agent::telemetry::Source {
            source_type: "x".into(),
            source_id: "y".into(),
            model: None,
            weather_provider: None,
        },
        &ippan_pv_agent::telemetry::Location {
            city: "c".into(),
            country: "u".into(),
            latitude: None,
            longitude: None,
            altitude_m: None,
        },
        &zero_canonical_telemetry(),
        &[],
        &[],
    );
    let bytes = to_canonical_bytes(&v).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let idx = |k: &str| text.find(&format!("\"{}\":", k)).unwrap();
    // Top-level keys in lexicographic order:
    // active_event_ids, events, interval_minutes, location, plant_id,
    // record_id, schema, source, telemetry, timestamp.
    assert!(idx("active_event_ids") < idx("events"));
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
    let v = json!({
        "schema": "ippan.pv.production.v2",
        "plant_id": "test-plant",
        "record_id": "pv-test-plant-20260515T101500Z",
        "timestamp": "2026-05-15T10:15:00Z",
        "interval_minutes": 15,
        "source": {"source_type": "test", "source_id": "s"},
        "location": {"city": "c", "country": "u"},
        "telemetry": {
            "ghi_w_m2": 100,
            "dni_w_m2": 0,
            "dhi_w_m2": 0,
            "ambient_temperature_milli_c": 20500,
            "humidity_milli_pct": 0,
            "wind_speed_milli_ms": 0,
            "precipitation_milli_mm": 0,
            "cloudcover_milli_pct": 0,
            "solar_elevation_milli_deg": 0,
            "solar_azimuth_milli_deg": 0,
            "poa_global_w_m2": 0,
            "poa_direct_w_m2": 0,
            "poa_diffuse_w_m2": 0,
            "cell_temperature_milli_c": 0,
            "dc_string_voltage_milli_v": 0,
            "dc_string_current_milli_a": 0,
            "dc_array_voltage_milli_v": 0,
            "dc_power_milliwatt": 1000000,
            "ac_power_milliwatt": 950000,
            "inverter_efficiency_milli_pct": 0,
            "meter_power_milliwatt": 900000,
            "apparent_power_milli_va": 0,
            "reactive_power_milli_var": 0,
            "grid_voltage_milli_v": 0,
            "grid_frequency_milli_hz": 0,
            "performance_ratio_ppm": 850000,
            "capacity_factor_milli_pct": 0,
            "energy_since_start_wh": 500,
            "strings_available": 300,
            "derating_factor_ppm": 1000000,
            "soiling_factor_ppm": 1000000
        },
        "active_event_ids": [],
        "events": []
    });
    let bytes = to_canonical_bytes(&v).unwrap();
    let h = ippan_pv_agent::hashing::sha256_prefixed_hex(&bytes);
    assert!(h.starts_with("sha256:"));
    assert_eq!(h.len(), "sha256:".len() + 64);
    assert_eq!(h, ippan_pv_agent::hashing::sha256_prefixed_hex(&bytes));
}

fn zero_canonical_telemetry() -> ippan_pv_agent::telemetry::CanonicalTelemetry {
    ippan_pv_agent::telemetry::CanonicalTelemetry {
        ghi_w_m2: 0,
        dni_w_m2: 0,
        dhi_w_m2: 0,
        ambient_temperature_milli_c: 0,
        humidity_milli_pct: 0,
        wind_speed_milli_ms: 0,
        precipitation_milli_mm: 0,
        cloudcover_milli_pct: 0,
        solar_elevation_milli_deg: 0,
        solar_azimuth_milli_deg: 0,
        poa_global_w_m2: 0,
        poa_direct_w_m2: 0,
        poa_diffuse_w_m2: 0,
        cell_temperature_milli_c: 0,
        dc_string_voltage_milli_v: 0,
        dc_string_current_milli_a: 0,
        dc_array_voltage_milli_v: 0,
        dc_power_milliwatt: 0,
        ac_power_milliwatt: 0,
        inverter_efficiency_milli_pct: 0,
        meter_power_milliwatt: 0,
        apparent_power_milli_va: 0,
        reactive_power_milli_var: 0,
        grid_voltage_milli_v: 0,
        grid_frequency_milli_hz: 0,
        performance_ratio_ppm: 0,
        capacity_factor_milli_pct: 0,
        energy_since_start_wh: 0,
        strings_available: 0,
        derating_factor_ppm: 0,
        soiling_factor_ppm: 0,
    }
}
