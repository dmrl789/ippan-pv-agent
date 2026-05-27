//! Operational events attached to a PV evidence record.
//!
//! Implements the IPPAN_DATA_SPECIFICATION v1.0 event schema: five event
//! types (matching IEC 62446 terminology), structured `affected_components`,
//! optional impact / spare-part / insurance / root-cause / soiling-reset
//! fields, and photo metadata.

use crate::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Five canonical event types per spec §3.1.
///
/// Note for downstream consumers: the old README example used the shorter
/// `maintenance` — that string is *not* accepted here. Use
/// `scheduled_maintenance` instead (IEC 62446 terminology and the
/// simulator's internal `events.yaml`).
pub const EVENT_TYPES: &[&str] = &[
    "scheduled_maintenance",
    "failure",
    "module_cleaning",
    "corrective_maintenance",
    "replacement",
];

/// Allowed values for `photos[].photo_type` (spec §3.5).
pub const PHOTO_TYPES: &[&str] = &[
    "pre_intervention",
    "in_progress",
    "post_intervention",
    "fault_finding",
    "diagnostics",
];

/// Allowed values for `status` (spec §3.3).
pub const EVENT_STATUSES: &[&str] = &["planned", "active", "completed", "cancelled"];

/// Maximum number of strings physically present at the plant
/// (10 inverters × 30 strings).
pub const MAX_STRINGS: i64 = 300;
pub const MAX_INVERTER_NUM: u32 = 10;
pub const MAX_STRING_NUM: u32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AffectedComponent {
    /// `inverter`, `string`, or `plant`.
    #[serde(rename = "type")]
    pub kind: String,
    /// `"INV-NN"`, `"INV-NN/STR-NN"`, or `"all"`.
    pub id: String,
    pub strings_offline: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Impact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strings_offline: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_reduction_pct: Option<i64>,
    /// Decimal string carrying the derating factor recorded at event start.
    /// Kept as a string so we never round-trip through `f64`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derating_at_start: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Photo {
    pub photo_id: String,
    pub filename: String,
    pub timestamp: String,
    pub photo_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub event_id: String,
    pub event_type: String,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    pub description: String,
    pub status: String,
    #[serde(default)]
    pub affected_components: Vec<AffectedComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<Impact>,
    pub operator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub photos: Vec<Photo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_cause: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spare_part: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insurance_claim: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soiling_reset: Option<bool>,
}

pub fn validate_event_type(t: &str) -> Result<()> {
    if EVENT_TYPES.contains(&t) {
        Ok(())
    } else {
        Err(Error::UnknownEventType(t.to_string()))
    }
}

pub fn validate_event_status(s: &str) -> Result<()> {
    if EVENT_STATUSES.contains(&s) {
        Ok(())
    } else {
        Err(Error::InvalidEvent(format!(
            "unknown status `{}` (expected one of {:?})",
            s, EVENT_STATUSES
        )))
    }
}

pub fn validate_photo_type(t: &str) -> Result<()> {
    if PHOTO_TYPES.contains(&t) {
        Ok(())
    } else {
        Err(Error::InvalidEvent(format!(
            "unknown photo_type `{}` (expected one of {:?})",
            t, PHOTO_TYPES
        )))
    }
}

pub fn parse_timestamp(s: &str) -> Result<DateTime<Utc>> {
    // Require ISO 8601 UTC with trailing Z.
    if !s.ends_with('Z') {
        return Err(Error::InvalidTimestamp(s.to_string()));
    }
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| Error::InvalidTimestamp(s.to_string()))
}

fn check_inverter_num(n_str: &str, ctx: &str) -> Result<()> {
    if n_str.len() != 2 || !n_str.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidEvent(format!(
            "invalid inverter number `{}` in {}",
            n_str, ctx
        )));
    }
    let n: u32 = n_str.parse().map_err(|_| {
        Error::InvalidEvent(format!("invalid inverter number `{}` in {}", n_str, ctx))
    })?;
    if !(1..=MAX_INVERTER_NUM).contains(&n) {
        return Err(Error::InvalidEvent(format!(
            "inverter number {} out of range [1..={}] in {}",
            n, MAX_INVERTER_NUM, ctx
        )));
    }
    Ok(())
}

fn check_string_num(n_str: &str, ctx: &str) -> Result<()> {
    if n_str.len() != 2 || !n_str.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidEvent(format!(
            "invalid string number `{}` in {}",
            n_str, ctx
        )));
    }
    let n: u32 = n_str.parse().map_err(|_| {
        Error::InvalidEvent(format!("invalid string number `{}` in {}", n_str, ctx))
    })?;
    if !(1..=MAX_STRING_NUM).contains(&n) {
        return Err(Error::InvalidEvent(format!(
            "string number {} out of range [1..={}] in {}",
            n, MAX_STRING_NUM, ctx
        )));
    }
    Ok(())
}

impl AffectedComponent {
    pub fn validate(&self) -> Result<()> {
        match self.kind.as_str() {
            "plant" => {
                if self.id != "all" {
                    return Err(Error::InvalidEvent(format!(
                        "plant-level component must have id=\"all\", got `{}`",
                        self.id
                    )));
                }
                if self.strings_offline < 0 || self.strings_offline > MAX_STRINGS {
                    return Err(Error::InvalidEvent(format!(
                        "strings_offline {} out of range [0..={}]",
                        self.strings_offline, MAX_STRINGS
                    )));
                }
            }
            "inverter" => {
                let rest = self.id.strip_prefix("INV-").ok_or_else(|| {
                    Error::InvalidEvent(format!(
                        "inverter id must look like `INV-NN`, got `{}`",
                        self.id
                    ))
                })?;
                check_inverter_num(rest, &format!("inverter id `{}`", self.id))?;
                if self.strings_offline < 0 || self.strings_offline > MAX_STRING_NUM as i64 {
                    return Err(Error::InvalidEvent(format!(
                        "inverter strings_offline {} out of range [0..={}]",
                        self.strings_offline, MAX_STRING_NUM
                    )));
                }
            }
            "string" => {
                let (inv_part, str_part) = self.id.split_once('/').ok_or_else(|| {
                    Error::InvalidEvent(format!(
                        "string id must look like `INV-NN/STR-NN`, got `{}`",
                        self.id
                    ))
                })?;
                let inv_num = inv_part.strip_prefix("INV-").ok_or_else(|| {
                    Error::InvalidEvent(format!(
                        "string id must start with `INV-NN/`, got `{}`",
                        self.id
                    ))
                })?;
                let str_num = str_part.strip_prefix("STR-").ok_or_else(|| {
                    Error::InvalidEvent(format!(
                        "string id must end with `/STR-NN`, got `{}`",
                        self.id
                    ))
                })?;
                check_inverter_num(inv_num, &format!("string id `{}`", self.id))?;
                check_string_num(str_num, &format!("string id `{}`", self.id))?;
                if self.strings_offline != 1 {
                    return Err(Error::InvalidEvent(format!(
                        "single-string component must have strings_offline=1, got {}",
                        self.strings_offline
                    )));
                }
            }
            other => {
                return Err(Error::InvalidEvent(format!(
                    "unknown component type `{}` (expected `plant`, `inverter`, or `string`)",
                    other
                )));
            }
        }
        Ok(())
    }
}

impl Photo {
    pub fn validate(&self) -> Result<()> {
        if self.photo_id.is_empty() {
            return Err(Error::InvalidEvent("photo_id is empty".into()));
        }
        if self.filename.is_empty() {
            return Err(Error::InvalidEvent("photo filename is empty".into()));
        }
        validate_photo_type(&self.photo_type)?;
        parse_timestamp(&self.timestamp)?;
        Ok(())
    }
}

impl Event {
    pub fn validate(&self) -> Result<()> {
        if self.event_id.is_empty() {
            return Err(Error::InvalidEvent("empty event_id".into()));
        }
        if self.operator.is_empty() {
            return Err(Error::InvalidEvent("empty operator".into()));
        }
        validate_event_type(&self.event_type)?;
        validate_event_status(&self.status)?;
        let started = parse_timestamp(&self.started_at)?;
        if let Some(ref e) = self.ended_at {
            let ended = parse_timestamp(e)?;
            if ended < started {
                return Err(Error::InvalidEvent(format!(
                    "event {}: ended_at < started_at",
                    self.event_id
                )));
            }
        }
        for c in &self.affected_components {
            c.validate()
                .map_err(|e| Error::InvalidEvent(format!("event {}: {}", self.event_id, e)))?;
        }
        for p in &self.photos {
            p.validate()
                .map_err(|e| Error::InvalidEvent(format!("event {}: {}", self.event_id, e)))?;
        }
        Ok(())
    }
}

/// Sort events deterministically: started_at, then event_id, then event_type.
pub fn sort_events(events: &mut [Event]) {
    events.sort_by(|a, b| {
        a.started_at
            .cmp(&b.started_at)
            .then(a.event_id.cmp(&b.event_id))
            .then(a.event_type.cmp(&b.event_type))
    });
}

/// Decide whether an event is "active or recent" relative to the record window.
pub fn should_attach(
    event: &Event,
    interval_start: DateTime<Utc>,
    interval_end: DateTime<Utc>,
    lookback_minutes: i64,
) -> Result<bool> {
    let started = parse_timestamp(&event.started_at)?;
    let ended = match &event.ended_at {
        Some(e) => Some(parse_timestamp(e)?),
        None => None,
    };

    if let Some(end) = ended {
        if end >= interval_start && started <= interval_end {
            return Ok(true);
        }
        let cutoff = interval_start - chrono::Duration::minutes(lookback_minutes);
        if end >= cutoff && end < interval_start {
            return Ok(true);
        }
        Ok(false)
    } else {
        Ok(started <= interval_end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> DateTime<Utc> {
        parse_timestamp(s).unwrap()
    }

    fn minimal_event(id: &str, t: &str, started: &str) -> Event {
        Event {
            event_id: id.into(),
            event_type: t.into(),
            started_at: started.into(),
            ended_at: None,
            description: "test".into(),
            status: "active".into(),
            affected_components: vec![],
            impact: None,
            operator: "op".into(),
            notes: None,
            photos: vec![],
            root_cause: None,
            spare_part: None,
            insurance_claim: None,
            soiling_reset: None,
        }
    }

    #[test]
    fn known_event_types() {
        for t in EVENT_TYPES {
            assert!(validate_event_type(t).is_ok());
        }
        assert!(validate_event_type("invented").is_err());
        // The old README value `maintenance` is explicitly NOT accepted.
        assert!(validate_event_type("maintenance").is_err());
    }

    #[test]
    fn validates_affected_components() {
        let plant = AffectedComponent {
            kind: "plant".into(),
            id: "all".into(),
            strings_offline: 150,
        };
        assert!(plant.validate().is_ok());

        let inv_ok = AffectedComponent {
            kind: "inverter".into(),
            id: "INV-03".into(),
            strings_offline: 30,
        };
        assert!(inv_ok.validate().is_ok());

        let inv_bad = AffectedComponent {
            kind: "inverter".into(),
            id: "INV-11".into(),
            strings_offline: 30,
        };
        assert!(inv_bad.validate().is_err());

        let str_ok = AffectedComponent {
            kind: "string".into(),
            id: "INV-05/STR-07".into(),
            strings_offline: 1,
        };
        assert!(str_ok.validate().is_ok());

        let str_bad = AffectedComponent {
            kind: "string".into(),
            id: "INV-05/STR-31".into(),
            strings_offline: 1,
        };
        assert!(str_bad.validate().is_err());

        let bad_plant = AffectedComponent {
            kind: "plant".into(),
            id: "some-plant".into(),
            strings_offline: 0,
        };
        assert!(bad_plant.validate().is_err());
    }

    #[test]
    fn validates_photo_types() {
        let good = Photo {
            photo_id: "P1".into(),
            filename: "x.jpg".into(),
            timestamp: "2026-05-15T10:00:00Z".into(),
            photo_type: "diagnostics".into(),
            description: None,
        };
        assert!(good.validate().is_ok());

        let bad = Photo {
            photo_id: "P1".into(),
            filename: "x.jpg".into(),
            timestamp: "2026-05-15T10:00:00Z".into(),
            photo_type: "selfie".into(),
            description: None,
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn sort_is_deterministic() {
        let mut a = vec![
            minimal_event("evt-3", "failure", "2026-05-15T09:00:00Z"),
            minimal_event("evt-1", "module_cleaning", "2026-05-15T08:00:00Z"),
            minimal_event("evt-2", "scheduled_maintenance", "2026-05-15T08:00:00Z"),
        ];
        sort_events(&mut a);
        assert_eq!(a[0].event_id, "evt-1");
        assert_eq!(a[1].event_id, "evt-2");
        assert_eq!(a[2].event_id, "evt-3");
    }

    #[test]
    fn attach_overlapping_event() {
        let start = ts("2026-05-15T10:15:00Z");
        let end = ts("2026-05-15T10:30:00Z");
        let mut e = minimal_event("x", "module_cleaning", "2026-05-15T10:00:00Z");
        e.ended_at = Some("2026-05-15T10:20:00Z".into());
        assert!(should_attach(&e, start, end, 240).unwrap());
    }

    #[test]
    fn attach_recent_event_via_lookback() {
        let start = ts("2026-05-15T10:15:00Z");
        let end = ts("2026-05-15T10:30:00Z");
        let mut e = minimal_event("x", "module_cleaning", "2026-05-15T08:00:00Z");
        e.ended_at = Some("2026-05-15T09:00:00Z".into());
        assert!(should_attach(&e, start, end, 240).unwrap());
    }

    #[test]
    fn skip_old_event_beyond_lookback() {
        let start = ts("2026-05-15T10:15:00Z");
        let end = ts("2026-05-15T10:30:00Z");
        let mut e = minimal_event("x", "module_cleaning", "2026-05-14T08:00:00Z");
        e.ended_at = Some("2026-05-14T09:00:00Z".into());
        assert!(!should_attach(&e, start, end, 240).unwrap());
    }

    #[test]
    fn reject_missing_z_suffix() {
        assert!(parse_timestamp("2026-05-15T10:15:00").is_err());
        assert!(parse_timestamp("2026-05-15T10:15:00+00:00").is_err());
        assert!(parse_timestamp("2026-05-15T10:15:00Z").is_ok());
    }
}
