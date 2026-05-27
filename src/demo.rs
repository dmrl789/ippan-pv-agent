//! Built-in Palermo 991 kWp demo data, aligned to IPPAN_DATA_SPECIFICATION v1.0.

use crate::events::{AffectedComponent, Event, Impact, Photo};
use crate::telemetry::{Location, RawInput, RawTelemetry, Source};

/// Produce a deterministic Palermo 991 kWp raw input matching the spec
/// example (timestamp 2026-05-20T12:15:00Z).
pub fn palermo_raw_input() -> RawInput {
    RawInput {
        plant_id: "palermo-pv-001".into(),
        timestamp: "2026-05-20T12:15:00Z".into(),
        interval_minutes: 15,
        location: Location {
            city: "Palermo".into(),
            country: "IT".into(),
            latitude: Some("38.1157".into()),
            longitude: Some("13.3615".into()),
            altitude_m: Some("14".into()),
        },
        source: Source {
            source_type: "pv_simulator".into(),
            source_id: "pvlib-openmeteo-palermo-v2".into(),
            model: Some("pvlib-CEC-Sandia".into()),
            weather_provider: Some("open-meteo".into()),
        },
        telemetry: RawTelemetry {
            ghi_w_m2: "554".into(),
            dni_w_m2: "680".into(),
            dhi_w_m2: "120".into(),
            ambient_temperature_c: "20.5".into(),
            cell_temperature_c: "36.20".into(),
            humidity_pct: "55.0".into(),
            wind_speed_ms: "3.200".into(),
            precipitation_mm: "0.00".into(),
            cloudcover_pct: "15.0".into(),
            solar_elevation_deg: "52.300".into(),
            solar_azimuth_deg: "185.400".into(),
            poa_global_w_m2: "612".into(),
            poa_direct_w_m2: "480".into(),
            poa_diffuse_w_m2: "132".into(),
            dc_string_voltage_v: "372.10".into(),
            dc_string_current_a: "8.880".into(),
            dc_array_voltage_v: "372.10".into(),
            dc_power_kw: "492.5000".into(),
            ac_power_kw: "471.6000".into(),
            inverter_efficiency_pct: "95.780".into(),
            meter_power_kw: "463.1000".into(),
            apparent_power_kva: "472.5510".into(),
            reactive_power_kvar: "94.8200".into(),
            grid_voltage_v: "400.0".into(),
            grid_frequency_hz: "50.0".into(),
            performance_ratio: "0.8590".into(),
            capacity_factor_pct: "47.588".into(),
            energy_since_start_kwh: "463.100".into(),
            strings_available: "300".into(),
            derating_factor: "1.0000".into(),
            soiling_factor: "0.9985".into(),
        },
        active_event_ids: vec![],
    }
}

/// Demo events list. Mirrors `examples/pv/palermo-events.json` byte-for-byte
/// (same fields, same order) so file-loaded and in-memory builds produce the
/// same canonical hash.
pub fn palermo_events() -> Vec<Event> {
    vec![Event {
        event_id: "EVT-001".into(),
        event_type: "scheduled_maintenance".into(),
        started_at: "2026-05-20T08:00:00Z".into(),
        ended_at: Some("2026-05-20T11:00:00Z".into()),
        description: "Annual inspection — inverter INV-03 (thermal imaging + torque check)".into(),
        status: "completed".into(),
        affected_components: vec![AffectedComponent {
            kind: "inverter".into(),
            id: "INV-03".into(),
            strings_offline: 30,
        }],
        impact: Some(Impact {
            strings_offline: Some(30),
            power_reduction_pct: Some(0),
            derating_at_start: Some("0.9000".into()),
        }),
        operator: "Mario Rossi".into(),
        notes: Some(
            "Annual preventive maintenance per IEC 62446. Visual inspection, IR thermography of all connectors, torque verification, AFCI self-test, firmware updated to v3.12, cooling fan replaced.".into(),
        ),
        photos: vec![
            Photo {
                photo_id: "P001-1".into(),
                filename: "docs/EVT-001_pre_intervention.jpg".into(),
                timestamp: "2026-05-20T07:55:00Z".into(),
                photo_type: "pre_intervention".into(),
                description: Some("INV-03 cabinet before maintenance".into()),
            },
            Photo {
                photo_id: "P001-2".into(),
                filename: "docs/EVT-001_ir_scan.jpg".into(),
                timestamp: "2026-05-20T09:30:00Z".into(),
                photo_type: "diagnostics".into(),
                description: Some("IR scan — no anomalies found on DC terminals".into()),
            },
            Photo {
                photo_id: "P001-3".into(),
                filename: "docs/EVT-001_post_intervention.jpg".into(),
                timestamp: "2026-05-20T10:50:00Z".into(),
                photo_type: "post_intervention".into(),
                description: Some(
                    "INV-03 after maintenance, firmware v3.12 confirmed".into(),
                ),
            },
        ],
        root_cause: None,
        spare_part: None,
        insurance_claim: None,
        soiling_reset: None,
    }]
}
