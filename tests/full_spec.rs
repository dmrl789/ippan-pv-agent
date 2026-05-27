//! Tests for IPPAN_DATA_SPECIFICATION v1.0 compliance.
//!
//! Each test pins one clause of the data contract so a future refactor that
//! quietly weakens validation will fail loudly.

mod common;

use ippan_pv_agent::bundle::{build_bundle, BuildOptions};
use ippan_pv_agent::demo::{palermo_events, palermo_raw_input};
use ippan_pv_agent::events::{AffectedComponent, Event, Impact, Photo};
use ippan_pv_agent::signing::OperatorKey;
use ippan_pv_agent::telemetry::RawInput;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn fixture(name: &str) -> PathBuf {
    let workspace = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(workspace).join("examples/pv").join(name)
}

fn read_input(name: &str) -> RawInput {
    let bytes = fs::read(fixture(name)).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn read_event(rel_path: &str) -> Event {
    let bytes = fs::read(fixture(rel_path)).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// --- Parser contract -------------------------------------------------------

#[test]
fn parses_full_telemetry_fixture() {
    let input = read_input("palermo-telemetry.json");
    assert_eq!(input.plant_id, "palermo-pv-001");
    assert_eq!(input.interval_minutes, 15);
    assert_eq!(input.location.city, "Palermo");
    assert_eq!(input.location.latitude.as_deref(), Some("38.1157"));
    assert_eq!(input.source.model.as_deref(), Some("pvlib-CEC-Sandia"));
    assert_eq!(input.source.weather_provider.as_deref(), Some("open-meteo"));
    assert_eq!(input.telemetry.ghi_w_m2, "554");
    assert_eq!(input.telemetry.strings_available, "300");
    assert_eq!(input.telemetry.soiling_factor, "0.9985");
    assert!(input.active_event_ids.is_empty());
}

#[test]
fn rejects_telemetry_numeric_fields_encoded_as_json_numbers() {
    let path = fixture("palermo-telemetry-invalid-json-numbers.json");
    let bytes = fs::read(&path).unwrap();
    let result: Result<RawInput, _> = serde_json::from_slice(&bytes);
    let err = result.expect_err("JSON-number telemetry must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("expected a string") || msg.contains("invalid type"),
        "unexpected parser error message: {}",
        msg
    );
}

#[test]
fn legacy_minimal_fixture_is_rejected_with_clear_error() {
    let path = fixture("palermo-telemetry-legacy-minimal.json");
    let bytes = fs::read(&path).unwrap();
    let result: Result<RawInput, _> = serde_json::from_slice(&bytes);
    let err = result.expect_err("legacy 7-field telemetry must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("missing field") || msg.contains("missing"),
        "expected a missing-field error for legacy minimal fixture, got: {}",
        msg
    );
}

// --- Event coverage --------------------------------------------------------

#[test]
fn parses_all_five_event_type_fixtures() {
    for (rel, expected_type) in [
        (
            "events/evt-scheduled-maintenance.json",
            "scheduled_maintenance",
        ),
        ("events/evt-failure.json", "failure"),
        ("events/evt-module-cleaning.json", "module_cleaning"),
        (
            "events/evt-corrective-maintenance.json",
            "corrective_maintenance",
        ),
        ("events/evt-replacement.json", "replacement"),
    ] {
        let event = read_event(rel);
        event.validate().expect(rel);
        assert_eq!(event.event_type, expected_type, "{}", rel);
    }
}

#[test]
fn scheduled_maintenance_is_accepted() {
    let event = read_event("events/evt-scheduled-maintenance.json");
    assert_eq!(event.event_type, "scheduled_maintenance");
    event
        .validate()
        .expect("scheduled_maintenance must validate");
}

#[test]
fn legacy_maintenance_event_type_is_rejected() {
    // `maintenance` was the pre-v1.0 README value. The new contract requires
    // `scheduled_maintenance`.
    let event = Event {
        event_id: "evt-old".into(),
        event_type: "maintenance".into(),
        started_at: "2026-05-10T06:00:00Z".into(),
        ended_at: Some("2026-05-10T07:00:00Z".into()),
        description: "legacy".into(),
        status: "completed".into(),
        affected_components: vec![],
        impact: None,
        operator: "op".into(),
        notes: None,
        photos: vec![],
        root_cause: None,
        spare_part: None,
        insurance_claim: None,
        soiling_reset: None,
    };
    assert!(event.validate().is_err());
}

// --- Structured affected_components ----------------------------------------

#[test]
fn validates_structured_affected_components() {
    let inverter = AffectedComponent {
        kind: "inverter".into(),
        id: "INV-10".into(),
        strings_offline: 30,
    };
    inverter.validate().unwrap();

    let string_ = AffectedComponent {
        kind: "string".into(),
        id: "INV-01/STR-30".into(),
        strings_offline: 1,
    };
    string_.validate().unwrap();

    let plant = AffectedComponent {
        kind: "plant".into(),
        id: "all".into(),
        strings_offline: 300,
    };
    plant.validate().unwrap();

    // Boundary failure: INV-11 does not exist (max INV-10).
    let bad_inv = AffectedComponent {
        kind: "inverter".into(),
        id: "INV-11".into(),
        strings_offline: 30,
    };
    assert!(bad_inv.validate().is_err());

    // Boundary failure: STR-31 does not exist (max STR-30).
    let bad_str = AffectedComponent {
        kind: "string".into(),
        id: "INV-05/STR-31".into(),
        strings_offline: 1,
    };
    assert!(bad_str.validate().is_err());

    // Unknown component type.
    let bad_type = AffectedComponent {
        kind: "transformer".into(),
        id: "T1".into(),
        strings_offline: 0,
    };
    assert!(bad_type.validate().is_err());
}

#[test]
fn validates_photo_type_whitelist() {
    for ok in [
        "pre_intervention",
        "in_progress",
        "post_intervention",
        "fault_finding",
        "diagnostics",
    ] {
        let p = Photo {
            photo_id: "x".into(),
            filename: "x.jpg".into(),
            timestamp: "2026-05-08T05:55:00Z".into(),
            photo_type: ok.into(),
            description: None,
        };
        p.validate().unwrap();
    }
    let bad = Photo {
        photo_id: "x".into(),
        filename: "x.jpg".into(),
        timestamp: "2026-05-08T05:55:00Z".into(),
        photo_type: "selfie".into(),
        description: None,
    };
    assert!(bad.validate().is_err());
}

// --- Active-event cross-reference and lookback ----------------------------

#[test]
fn attaches_active_event_ids_correctly() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");

    let mut raw = palermo_raw_input();
    let mut events = palermo_events();
    // Add an old event that the 240-min lookback would normally drop, then
    // force it active to confirm active_event_ids overrides the lookback.
    events.push(Event {
        event_id: "EVT-OLD".into(),
        event_type: "module_cleaning".into(),
        started_at: "2026-05-08T04:00:00Z".into(),
        ended_at: Some("2026-05-08T09:00:00Z".into()),
        description: "old cleaning, normally beyond lookback".into(),
        status: "completed".into(),
        affected_components: vec![AffectedComponent {
            kind: "plant".into(),
            id: "all".into(),
            strings_offline: 0,
        }],
        impact: None,
        operator: "Cleaning Crew".into(),
        notes: None,
        photos: vec![],
        root_cause: None,
        spare_part: None,
        insurance_claim: None,
        soiling_reset: Some(true),
    });
    raw.active_event_ids = vec!["EVT-OLD".into()];

    let built = build_bundle(&cfg, &raw, &events, &key, &BuildOptions::default()).unwrap();
    let bytes = fs::read(built.bundle_dir.join("canonical-record.json")).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let active = v["active_event_ids"].as_array().unwrap();
    assert!(active.iter().any(|x| x == "EVT-OLD"));

    let attached_ids: Vec<String> = v["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["event_id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        attached_ids.contains(&"EVT-OLD".to_string()),
        "active event must always be attached even outside lookback, got {:?}",
        attached_ids
    );
}

#[test]
fn attaches_recent_completed_events_via_lookback() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");

    // Build an event that ended 60 minutes before the demo timestamp
    // (2026-05-20T12:15:00Z). 60 min < 240 min lookback → must attach.
    let mut events = vec![Event {
        event_id: "EVT-RECENT".into(),
        event_type: "module_cleaning".into(),
        started_at: "2026-05-20T10:00:00Z".into(),
        ended_at: Some("2026-05-20T11:15:00Z".into()),
        description: "recent cleaning".into(),
        status: "completed".into(),
        affected_components: vec![AffectedComponent {
            kind: "plant".into(),
            id: "all".into(),
            strings_offline: 0,
        }],
        impact: None,
        operator: "Cleaning Crew".into(),
        notes: None,
        photos: vec![],
        root_cause: None,
        spare_part: None,
        insurance_claim: None,
        soiling_reset: Some(true),
    }];
    // Add a too-old event that must NOT be attached.
    events.push(Event {
        event_id: "EVT-ANCIENT".into(),
        event_type: "scheduled_maintenance".into(),
        started_at: "2026-05-15T06:00:00Z".into(),
        ended_at: Some("2026-05-15T08:00:00Z".into()),
        description: "old job".into(),
        status: "completed".into(),
        affected_components: vec![],
        impact: None,
        operator: "Mario".into(),
        notes: None,
        photos: vec![],
        root_cause: None,
        spare_part: None,
        insurance_claim: None,
        soiling_reset: None,
    });

    let built = build_bundle(
        &cfg,
        &palermo_raw_input(),
        &events,
        &key,
        &BuildOptions::default(),
    )
    .unwrap();
    let bytes = fs::read(built.bundle_dir.join("canonical-record.json")).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let attached_ids: Vec<String> = v["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["event_id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        attached_ids.contains(&"EVT-RECENT".to_string()),
        "recent event must be attached (240-min lookback): {:?}",
        attached_ids
    );
    assert!(
        !attached_ids.contains(&"EVT-ANCIENT".to_string()),
        "ancient event must NOT be attached: {:?}",
        attached_ids
    );
}

#[test]
fn rejects_unknown_event_id_in_active_event_ids() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");

    let mut raw = palermo_raw_input();
    raw.active_event_ids = vec!["EVT-DOES-NOT-EXIST".into()];

    let err = build_bundle(
        &cfg,
        &raw,
        &palermo_events(),
        &key,
        &BuildOptions::default(),
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("EVT-DOES-NOT-EXIST"),
        "expected unknown-id error, got: {}",
        msg
    );
}

// --- Canonical hashing stability ------------------------------------------

#[test]
fn canonical_hash_is_stable_across_repeated_loads() {
    let dir1 = tempdir().unwrap();
    let cfg1 = common::make_config(dir1.path());
    let key = OperatorKey::generate_demo("key:test");
    let raw = palermo_raw_input();
    let events = palermo_events();

    let a = build_bundle(&cfg1, &raw, &events, &key, &BuildOptions::default()).unwrap();

    // Re-parse the canonical record from disk and re-build from a fresh
    // RawInput parsed out of the example file. The canonical hash must match
    // both invocations regardless of intermediate string parsing.
    let raw_again: RawInput =
        serde_json::from_slice(&fs::read(fixture("palermo-telemetry.json")).unwrap()).unwrap();
    let events_again: Vec<Event> =
        serde_json::from_slice(&fs::read(fixture("palermo-events.json")).unwrap()).unwrap();

    let dir2 = tempdir().unwrap();
    let mut cfg2 = cfg1.clone();
    cfg2.storage.base_dir = dir2.path().to_string_lossy().into_owned();
    let b = build_bundle(
        &cfg2,
        &raw_again,
        &events_again,
        &key,
        &BuildOptions::default(),
    )
    .unwrap();
    assert_eq!(a.canonical_hash, b.canonical_hash);

    // And a third construction from a hand-built RawInput identical to the
    // example yields the same hash.
    let dir3 = tempdir().unwrap();
    let mut cfg3 = cfg1.clone();
    cfg3.storage.base_dir = dir3.path().to_string_lossy().into_owned();
    let c = build_bundle(&cfg3, &raw, &events, &key, &BuildOptions::default()).unwrap();
    assert_eq!(a.canonical_hash, c.canonical_hash);
}

// --- Bundle includes O&M fields and source/location blocks ----------------

#[test]
fn evidence_record_includes_om_fields_source_location_active_ids() {
    let dir = tempdir().unwrap();
    let cfg = common::make_config(dir.path());
    let key = OperatorKey::generate_demo("key:test");
    let raw = palermo_raw_input();
    let built = build_bundle(&cfg, &raw, &[], &key, &BuildOptions::default()).unwrap();

    let bytes = fs::read(built.bundle_dir.join("canonical-record.json")).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    // O&M fields present in canonical telemetry.
    for k in [
        "strings_available",
        "derating_factor_ppm",
        "soiling_factor_ppm",
    ] {
        assert!(
            v["telemetry"].get(k).is_some(),
            "telemetry missing O&M field {}",
            k
        );
    }

    // Source block extended.
    assert_eq!(v["source"]["source_type"], "pv_simulator");
    assert_eq!(v["source"]["model"], "pvlib-CEC-Sandia");
    assert_eq!(v["source"]["weather_provider"], "open-meteo");

    // Location block extended.
    assert_eq!(v["location"]["city"], "Palermo");
    assert_eq!(v["location"]["latitude"], "38.1157");
    assert_eq!(v["location"]["longitude"], "13.3615");
    assert_eq!(v["location"]["altitude_m"], "14");

    // active_event_ids present (empty in this case).
    assert!(v["active_event_ids"].is_array());

    let _impact = Impact {
        strings_offline: Some(0),
        power_reduction_pct: Some(0),
        derating_at_start: None,
    };
}
