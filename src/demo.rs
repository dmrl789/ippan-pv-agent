//! Built-in Palermo 1MW demo data.

use crate::events::Event;
use crate::telemetry::{Location, RawInput, RawTelemetry, Source};

/// Produce a deterministic Palermo 1MW raw input.
pub fn palermo_raw_input() -> RawInput {
    RawInput {
        plant_id: "palermo-pv-001".into(),
        timestamp: "2026-05-15T10:15:00Z".into(),
        interval_minutes: 15,
        location: Location {
            city: "Palermo".into(),
            country: "IT".into(),
        },
        source: Source {
            source_type: "pv_simulator".into(),
            source_id: "desiree-palermo-sim-v1".into(),
        },
        telemetry: RawTelemetry {
            ghi_w_m2: "554".into(),
            ambient_temperature_c: "20.5".into(),
            dc_power_kw: "492.5".into(),
            ac_power_kw: "471.6".into(),
            meter_power_kw: "463.1".into(),
            performance_ratio: "0.859".into(),
            energy_since_start_kwh: "463.1".into(),
        },
    }
}

pub fn palermo_events() -> Vec<Event> {
    vec![Event {
        event_id: "evt-20260515-001".into(),
        event_type: "module_cleaning".into(),
        started_at: "2026-05-15T08:00:00Z".into(),
        ended_at: Some("2026-05-15T09:00:00Z".into()),
        description: "Routine module cleaning completed".into(),
        affected_components: vec!["pv_string_01".into(), "pv_string_02".into()],
        operator: "operator-id-or-key-ref".into(),
    }]
}
