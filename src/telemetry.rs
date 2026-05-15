//! PV telemetry: raw input → scaled-integer canonical telemetry.
//!
//! Determinism rule: never round-trip through `f64`. All decimal inputs are
//! parsed from strings and scaled by integer arithmetic.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub city: String,
    pub country: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub source_type: String,
    pub source_id: String,
}

/// Raw PV telemetry as it comes from the simulator/meter bridge.
/// Decimal values are strings to avoid binary-float ambiguity in input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTelemetry {
    pub ghi_w_m2: String,
    pub ambient_temperature_c: String,
    pub dc_power_kw: String,
    pub ac_power_kw: String,
    pub meter_power_kw: String,
    pub performance_ratio: String,
    pub energy_since_start_kwh: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawInput {
    pub plant_id: String,
    pub timestamp: String,
    pub interval_minutes: u32,
    pub location: Location,
    pub source: Source,
    pub telemetry: RawTelemetry,
}

/// Canonical telemetry — integer-only, ready to hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalTelemetry {
    pub ghi_w_m2: i64,
    pub ambient_temperature_milli_c: i64,
    pub dc_power_w: i64,
    pub ac_power_w: i64,
    pub meter_power_w: i64,
    pub performance_ratio_ppm: i64,
    pub energy_since_start_wh: i64,
}

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

    // pad fractional part on the right to `scale_decimals` digits
    let pad = scale_decimals as usize - frac_part.len();
    for _ in 0..pad {
        frac_val = frac_val.checked_mul(10).ok_or_else(|| Error::InvalidDecimal {
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

    Ok(sign.checked_mul(scaled).ok_or_else(|| Error::InvalidDecimal {
        field: field.into(),
        value: input.into(),
    })?)
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

impl RawTelemetry {
    pub fn to_canonical(&self) -> Result<CanonicalTelemetry> {
        // GHI: integer W/m². Scale 0. Non-negative.
        let ghi = parse_scaled("ghi_w_m2", &self.ghi_w_m2, 0)?;
        let ghi = require_non_negative("ghi_w_m2", &self.ghi_w_m2, ghi)?;

        // Ambient temperature can be negative.
        let amb = parse_scaled("ambient_temperature_c", &self.ambient_temperature_c, 3)?;

        let dc = parse_scaled("dc_power_kw", &self.dc_power_kw, 3)?;
        let dc = require_non_negative("dc_power_kw", &self.dc_power_kw, dc)?;

        let ac = parse_scaled("ac_power_kw", &self.ac_power_kw, 3)?;
        let ac = require_non_negative("ac_power_kw", &self.ac_power_kw, ac)?;

        let meter = parse_scaled("meter_power_kw", &self.meter_power_kw, 3)?;
        let meter = require_non_negative("meter_power_kw", &self.meter_power_kw, meter)?;

        let pr = parse_scaled("performance_ratio", &self.performance_ratio, 6)?;
        let pr = require_non_negative("performance_ratio", &self.performance_ratio, pr)?;

        let e = parse_scaled("energy_since_start_kwh", &self.energy_since_start_kwh, 3)?;
        let e = require_non_negative("energy_since_start_kwh", &self.energy_since_start_kwh, e)?;

        Ok(CanonicalTelemetry {
            ghi_w_m2: ghi,
            ambient_temperature_milli_c: amb,
            dc_power_w: dc,
            ac_power_w: ac,
            meter_power_w: meter,
            performance_ratio_ppm: pr,
            energy_since_start_wh: e,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn palermo_example_converts() {
        let raw = RawTelemetry {
            ghi_w_m2: "554".into(),
            ambient_temperature_c: "20.5".into(),
            dc_power_kw: "492.5".into(),
            ac_power_kw: "471.6".into(),
            meter_power_kw: "463.1".into(),
            performance_ratio: "0.859".into(),
            energy_since_start_kwh: "463.1".into(),
        };
        let c = raw.to_canonical().unwrap();
        assert_eq!(c.ghi_w_m2, 554);
        assert_eq!(c.ambient_temperature_milli_c, 20_500);
        assert_eq!(c.dc_power_w, 492_500);
        assert_eq!(c.ac_power_w, 471_600);
        assert_eq!(c.meter_power_w, 463_100);
        assert_eq!(c.performance_ratio_ppm, 859_000);
        assert_eq!(c.energy_since_start_wh, 463_100);
    }
}
