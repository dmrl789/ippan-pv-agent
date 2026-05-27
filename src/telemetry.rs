//! PV telemetry: raw input → scaled-integer canonical telemetry.
//!
//! Determinism rule: never round-trip through `f64`. All decimal inputs are
//! parsed from strings and scaled by integer arithmetic.
//!
//! This module implements the IPPAN_DATA_SPECIFICATION v1.0 simulator
//! contract: 31 telemetry fields, extended location/source blocks, and an
//! `active_event_ids` array. All numeric telemetry values are JSON strings.
//! Bare JSON numbers are rejected by serde at parse time, which preserves
//! deterministic canonical encoding.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Location block — extended in v1.0 with latitude / longitude / altitude.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub city: String,
    pub country: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latitude: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub longitude: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub altitude_m: Option<String>,
}

/// Source block — extended in v1.0 with `model` and `weather_provider`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub source_type: String,
    pub source_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weather_provider: Option<String>,
}

/// Raw PV telemetry from the simulator/meter bridge. Every numeric field is
/// a decimal string — bare JSON numbers are rejected by serde at parse time.
///
/// 31 fields, mirroring the PVSimulator Palermo 991 kWp data contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTelemetry {
    // Meteorological
    pub ghi_w_m2: String,
    pub dni_w_m2: String,
    pub dhi_w_m2: String,
    pub ambient_temperature_c: String,
    pub humidity_pct: String,
    pub wind_speed_ms: String,
    pub precipitation_mm: String,
    pub cloudcover_pct: String,

    // Solar geometry
    pub solar_elevation_deg: String,
    pub solar_azimuth_deg: String,

    // Plane-of-array irradiance
    pub poa_global_w_m2: String,
    pub poa_direct_w_m2: String,
    pub poa_diffuse_w_m2: String,

    // Module / cell
    pub cell_temperature_c: String,

    // DC field
    pub dc_string_voltage_v: String,
    pub dc_string_current_a: String,
    pub dc_array_voltage_v: String,
    pub dc_power_kw: String,

    // AC output
    pub ac_power_kw: String,
    pub inverter_efficiency_pct: String,

    // Grid / power meter
    pub meter_power_kw: String,
    pub apparent_power_kva: String,
    pub reactive_power_kvar: String,
    pub grid_voltage_v: String,
    pub grid_frequency_hz: String,

    // Performance indices
    pub performance_ratio: String,
    pub capacity_factor_pct: String,

    // Energy accumulator
    pub energy_since_start_kwh: String,

    // O&M state
    pub strings_available: String,
    pub derating_factor: String,
    pub soiling_factor: String,
}

/// Top-level raw input one timestep — telemetry plus context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawInput {
    pub plant_id: String,
    pub timestamp: String,
    pub interval_minutes: u32,
    pub location: Location,
    pub source: Source,
    pub telemetry: RawTelemetry,
    #[serde(default)]
    pub active_event_ids: Vec<String>,
}

/// Canonical telemetry — integer-only, ready to hash.
///
/// Scale rules: powers (kW input) go to milliwatt (scale 6 → mW), voltages /
/// currents / temperatures / percentages with up to 3 dp use scale 3, ratios
/// use parts-per-million (scale 6), irradiance and counts are integer (scale
/// 0). Energy keeps the existing scale-3 watt-hour encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalTelemetry {
    pub ghi_w_m2: i64,
    pub dni_w_m2: i64,
    pub dhi_w_m2: i64,
    pub ambient_temperature_milli_c: i64,
    pub humidity_milli_pct: i64,
    pub wind_speed_milli_ms: i64,
    pub precipitation_milli_mm: i64,
    pub cloudcover_milli_pct: i64,
    pub solar_elevation_milli_deg: i64,
    pub solar_azimuth_milli_deg: i64,
    pub poa_global_w_m2: i64,
    pub poa_direct_w_m2: i64,
    pub poa_diffuse_w_m2: i64,
    pub cell_temperature_milli_c: i64,
    pub dc_string_voltage_milli_v: i64,
    pub dc_string_current_milli_a: i64,
    pub dc_array_voltage_milli_v: i64,
    pub dc_power_milliwatt: i64,
    pub ac_power_milliwatt: i64,
    pub inverter_efficiency_milli_pct: i64,
    pub meter_power_milliwatt: i64,
    pub apparent_power_milli_va: i64,
    pub reactive_power_milli_var: i64,
    pub grid_voltage_milli_v: i64,
    pub grid_frequency_milli_hz: i64,
    pub performance_ratio_ppm: i64,
    pub capacity_factor_milli_pct: i64,
    pub energy_since_start_wh: i64,
    pub strings_available: i64,
    pub derating_factor_ppm: i64,
    pub soiling_factor_ppm: i64,
}

/// Maximum number of strings physically present at the Palermo plant
/// (10 inverters × 30 strings).
pub const MAX_STRINGS_AVAILABLE: i64 = 300;

/// Parse a decimal string scaled by `10^scale_decimals`, returning an i64.
///
/// - Accepts optional leading sign.
/// - Accepts integer part, optional fractional part with at most
///   `scale_decimals` digits.
/// - Rejects empty, malformed, or over-long fractional parts.
/// - Performs all arithmetic with checked integer ops (no float).
pub fn parse_scaled(field: &str, input: &str, scale_decimals: u32) -> Result<i64> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(Error::InvalidDecimal {
            field: field.into(),
            value: input.into(),
        });
    }

    let (sign, rest) = if let Some(s) = raw.strip_prefix('-') {
        (-1i64, s)
    } else if let Some(s) = raw.strip_prefix('+') {
        (1i64, s)
    } else {
        (1i64, raw)
    };

    let (int_part, frac_part) = match rest.split_once('.') {
        Some((i, f)) => (i, f),
        None => (rest, ""),
    };

    if int_part.is_empty() && frac_part.is_empty() {
        return Err(Error::InvalidDecimal {
            field: field.into(),
            value: input.into(),
        });
    }
    if !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidDecimal {
            field: field.into(),
            value: input.into(),
        });
    }
    if !frac_part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidDecimal {
            field: field.into(),
            value: input.into(),
        });
    }
    if frac_part.len() > scale_decimals as usize {
        return Err(Error::InvalidDecimal {
            field: field.into(),
            value: input.into(),
        });
    }

    let int_val: i64 = if int_part.is_empty() {
        0
    } else {
        int_part.parse().map_err(|_| Error::InvalidDecimal {
            field: field.into(),
            value: input.into(),
        })?
    };

    let mut frac_val: i64 = if frac_part.is_empty() {
        0
    } else {
        frac_part.parse().map_err(|_| Error::InvalidDecimal {
            field: field.into(),
            value: input.into(),
        })?
    };

    let pad = scale_decimals as usize - frac_part.len();
    for _ in 0..pad {
        frac_val = frac_val
            .checked_mul(10)
            .ok_or_else(|| Error::InvalidDecimal {
                field: field.into(),
                value: input.into(),
            })?;
    }

    let scale = 10i64
        .checked_pow(scale_decimals)
        .ok_or_else(|| Error::InvalidDecimal {
            field: field.into(),
            value: input.into(),
        })?;
    let scaled = int_val
        .checked_mul(scale)
        .and_then(|v| v.checked_add(frac_val))
        .ok_or_else(|| Error::InvalidDecimal {
            field: field.into(),
            value: input.into(),
        })?;

    sign.checked_mul(scaled)
        .ok_or_else(|| Error::InvalidDecimal {
            field: field.into(),
            value: input.into(),
        })
}

fn require_non_negative(field: &str, raw: &str, value: i64) -> Result<i64> {
    if value < 0 {
        Err(Error::NegativeValue {
            field: field.into(),
            value: raw.into(),
        })
    } else {
        Ok(value)
    }
}

fn parse_nn(field: &str, raw: &str, scale: u32) -> Result<i64> {
    let v = parse_scaled(field, raw, scale)?;
    require_non_negative(field, raw, v)
}

impl RawTelemetry {
    pub fn to_canonical(&self) -> Result<CanonicalTelemetry> {
        // Irradiance — integer W/m², non-negative.
        let ghi = parse_nn("ghi_w_m2", &self.ghi_w_m2, 0)?;
        let dni = parse_nn("dni_w_m2", &self.dni_w_m2, 0)?;
        let dhi = parse_nn("dhi_w_m2", &self.dhi_w_m2, 0)?;
        let poa_g = parse_nn("poa_global_w_m2", &self.poa_global_w_m2, 0)?;
        let poa_d = parse_nn("poa_direct_w_m2", &self.poa_direct_w_m2, 0)?;
        let poa_diff = parse_nn("poa_diffuse_w_m2", &self.poa_diffuse_w_m2, 0)?;

        // Temperatures may be negative.
        let amb = parse_scaled("ambient_temperature_c", &self.ambient_temperature_c, 3)?;
        let cell = parse_scaled("cell_temperature_c", &self.cell_temperature_c, 3)?;

        // Percentages 0..100 (we allow up to spec precision; non-negative).
        let hum = parse_nn("humidity_pct", &self.humidity_pct, 3)?;
        let cloud = parse_nn("cloudcover_pct", &self.cloudcover_pct, 3)?;
        let inv_eff = parse_nn("inverter_efficiency_pct", &self.inverter_efficiency_pct, 3)?;
        let cap_f = parse_nn("capacity_factor_pct", &self.capacity_factor_pct, 3)?;

        // Wind and rain non-negative.
        let wind = parse_nn("wind_speed_ms", &self.wind_speed_ms, 3)?;
        let precip = parse_nn("precipitation_mm", &self.precipitation_mm, 3)?;

        // Solar geometry — elevation can go below zero at night.
        let elev = parse_scaled("solar_elevation_deg", &self.solar_elevation_deg, 3)?;
        let azim = parse_nn("solar_azimuth_deg", &self.solar_azimuth_deg, 3)?;

        // DC field — non-negative.
        let dc_v = parse_nn("dc_string_voltage_v", &self.dc_string_voltage_v, 3)?;
        let dc_i = parse_nn("dc_string_current_a", &self.dc_string_current_a, 3)?;
        let dc_arr_v = parse_nn("dc_array_voltage_v", &self.dc_array_voltage_v, 3)?;
        let dc_p = parse_nn("dc_power_kw", &self.dc_power_kw, 6)?;

        // AC + meter — non-negative.
        let ac_p = parse_nn("ac_power_kw", &self.ac_power_kw, 6)?;
        let meter_p = parse_nn("meter_power_kw", &self.meter_power_kw, 6)?;
        let app = parse_nn("apparent_power_kva", &self.apparent_power_kva, 6)?;
        // Reactive power can be positive or negative; allow signed.
        let react = parse_scaled("reactive_power_kvar", &self.reactive_power_kvar, 6)?;

        // Grid quality — non-negative.
        let grid_v = parse_nn("grid_voltage_v", &self.grid_voltage_v, 3)?;
        let grid_f = parse_nn("grid_frequency_hz", &self.grid_frequency_hz, 3)?;

        // Ratios — non-negative.
        let pr = parse_nn("performance_ratio", &self.performance_ratio, 6)?;
        let energy = parse_nn("energy_since_start_kwh", &self.energy_since_start_kwh, 3)?;

        // O&M state.
        let strings = parse_nn("strings_available", &self.strings_available, 0)?;
        if strings > MAX_STRINGS_AVAILABLE {
            return Err(Error::InvalidDecimal {
                field: "strings_available".into(),
                value: self.strings_available.clone(),
            });
        }
        let derate = parse_nn("derating_factor", &self.derating_factor, 6)?;
        let soil = parse_nn("soiling_factor", &self.soiling_factor, 6)?;

        Ok(CanonicalTelemetry {
            ghi_w_m2: ghi,
            dni_w_m2: dni,
            dhi_w_m2: dhi,
            ambient_temperature_milli_c: amb,
            humidity_milli_pct: hum,
            wind_speed_milli_ms: wind,
            precipitation_milli_mm: precip,
            cloudcover_milli_pct: cloud,
            solar_elevation_milli_deg: elev,
            solar_azimuth_milli_deg: azim,
            poa_global_w_m2: poa_g,
            poa_direct_w_m2: poa_d,
            poa_diffuse_w_m2: poa_diff,
            cell_temperature_milli_c: cell,
            dc_string_voltage_milli_v: dc_v,
            dc_string_current_milli_a: dc_i,
            dc_array_voltage_milli_v: dc_arr_v,
            dc_power_milliwatt: dc_p,
            ac_power_milliwatt: ac_p,
            inverter_efficiency_milli_pct: inv_eff,
            meter_power_milliwatt: meter_p,
            apparent_power_milli_va: app,
            reactive_power_milli_var: react,
            grid_voltage_milli_v: grid_v,
            grid_frequency_milli_hz: grid_f,
            performance_ratio_ppm: pr,
            capacity_factor_milli_pct: cap_f,
            energy_since_start_wh: energy,
            strings_available: strings,
            derating_factor_ppm: derate,
            soiling_factor_ppm: soil,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palermo_raw() -> RawTelemetry {
        RawTelemetry {
            ghi_w_m2: "554".into(),
            dni_w_m2: "680".into(),
            dhi_w_m2: "120".into(),
            ambient_temperature_c: "20.5".into(),
            humidity_pct: "55.0".into(),
            wind_speed_ms: "3.200".into(),
            precipitation_mm: "0.00".into(),
            cloudcover_pct: "15.0".into(),
            solar_elevation_deg: "52.300".into(),
            solar_azimuth_deg: "185.400".into(),
            poa_global_w_m2: "612".into(),
            poa_direct_w_m2: "480".into(),
            poa_diffuse_w_m2: "132".into(),
            cell_temperature_c: "36.20".into(),
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
        }
    }

    #[test]
    fn parse_integer() {
        assert_eq!(parse_scaled("f", "554", 0).unwrap(), 554);
        assert_eq!(parse_scaled("f", "554", 3).unwrap(), 554_000);
    }

    #[test]
    fn parse_decimal() {
        assert_eq!(parse_scaled("f", "492.5", 3).unwrap(), 492_500);
        assert_eq!(parse_scaled("f", "20.5", 3).unwrap(), 20_500);
        assert_eq!(parse_scaled("f", "0.859", 6).unwrap(), 859_000);
        assert_eq!(parse_scaled("f", "463.1", 3).unwrap(), 463_100);
    }

    #[test]
    fn parse_negative() {
        assert_eq!(parse_scaled("f", "-1.5", 3).unwrap(), -1_500);
    }

    #[test]
    fn parse_padding_works() {
        assert_eq!(parse_scaled("f", "1.2", 3).unwrap(), 1_200);
        assert_eq!(parse_scaled("f", "1.20", 3).unwrap(), 1_200);
        assert_eq!(parse_scaled("f", "1.200", 3).unwrap(), 1_200);
    }

    #[test]
    fn reject_too_many_decimals() {
        assert!(parse_scaled("f", "1.2345", 3).is_err());
    }

    #[test]
    fn reject_garbage() {
        assert!(parse_scaled("f", "abc", 3).is_err());
        assert!(parse_scaled("f", "1.2e3", 3).is_err());
        assert!(parse_scaled("f", "", 3).is_err());
    }

    #[test]
    fn palermo_full_example_converts() {
        let raw = palermo_raw();
        let c = raw.to_canonical().unwrap();
        assert_eq!(c.ghi_w_m2, 554);
        assert_eq!(c.dni_w_m2, 680);
        assert_eq!(c.dhi_w_m2, 120);
        assert_eq!(c.ambient_temperature_milli_c, 20_500);
        assert_eq!(c.cell_temperature_milli_c, 36_200);
        assert_eq!(c.dc_power_milliwatt, 492_500_000);
        assert_eq!(c.ac_power_milliwatt, 471_600_000);
        assert_eq!(c.meter_power_milliwatt, 463_100_000);
        assert_eq!(c.performance_ratio_ppm, 859_000);
        assert_eq!(c.energy_since_start_wh, 463_100);
        assert_eq!(c.strings_available, 300);
        assert_eq!(c.derating_factor_ppm, 1_000_000);
        assert_eq!(c.soiling_factor_ppm, 998_500);
    }

    #[test]
    fn reject_strings_available_above_max() {
        let mut raw = palermo_raw();
        raw.strings_available = "301".into();
        assert!(raw.to_canonical().is_err());
    }

    #[test]
    fn reject_negative_for_required_non_negative_fields() {
        let mut raw = palermo_raw();
        raw.ghi_w_m2 = "-1".into();
        assert!(matches!(
            raw.to_canonical().unwrap_err(),
            Error::NegativeValue { .. }
        ));
    }

    #[test]
    fn integer_overflow_is_caught_not_silently_wrapped() {
        assert!(parse_scaled("f", "99999999999999999999", 3).is_err());
        assert!(parse_scaled("f", "9999999999999.999", 6).is_err());
    }

    #[test]
    fn raw_telemetry_rejects_json_numbers() {
        // The spec is explicit: numeric telemetry values must be JSON strings.
        // serde with `String` fields refuses bare JSON number literals — this
        // test pins that contract.
        let body = r#"{
            "ghi_w_m2": 554,
            "dni_w_m2": "680",
            "dhi_w_m2": "120",
            "ambient_temperature_c": "20.5",
            "humidity_pct": "55.0",
            "wind_speed_ms": "3.200",
            "precipitation_mm": "0.00",
            "cloudcover_pct": "15.0",
            "solar_elevation_deg": "52.300",
            "solar_azimuth_deg": "185.400",
            "poa_global_w_m2": "612",
            "poa_direct_w_m2": "480",
            "poa_diffuse_w_m2": "132",
            "cell_temperature_c": "36.20",
            "dc_string_voltage_v": "372.10",
            "dc_string_current_a": "8.880",
            "dc_array_voltage_v": "372.10",
            "dc_power_kw": "492.5000",
            "ac_power_kw": "471.6000",
            "inverter_efficiency_pct": "95.780",
            "meter_power_kw": "463.1000",
            "apparent_power_kva": "472.5510",
            "reactive_power_kvar": "94.8200",
            "grid_voltage_v": "400.0",
            "grid_frequency_hz": "50.0",
            "performance_ratio": "0.8590",
            "capacity_factor_pct": "47.588",
            "energy_since_start_kwh": "463.100",
            "strings_available": "300",
            "derating_factor": "1.0000",
            "soiling_factor": "0.9985"
        }"#;
        let err = serde_json::from_str::<RawTelemetry>(body).unwrap_err();
        assert!(
            err.to_string().contains("expected a string")
                || err.to_string().contains("invalid type"),
            "unexpected error message: {}",
            err
        );
    }
}
