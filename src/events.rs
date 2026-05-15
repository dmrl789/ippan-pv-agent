//! Operational events attached to a PV evidence record.

use crate::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const EVENT_TYPES: &[&str] = &[
    "scheduled_maintenance",
    "failure",
    "module_cleaning",
    "corrective_maintenance",
    "replacement",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub event_id: String,
    pub event_type: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub description: String,
    #[serde(default)]
    pub affected_components: Vec<String>,
    pub operator: String,
}

pub fn validate_event_type(t: &str) -> Result<()> {
    if EVENT_TYPES.contains(&t) {
        Ok(())
    } else {
        Err(Error::UnknownEventType(t.to_string()))
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

impl Event {
    pub fn validate(&self) -> Result<()> {
        validate_event_type(&self.event_type)?;
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
        if self.event_id.is_empty() {
            return Err(Error::InvalidEvent("empty event_id".into()));
        }
        if self.operator.is_empty() {
            return Err(Error::InvalidEvent("empty operator".into()));
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
///
/// The record covers [interval_start, interval_end). An event is attached if
/// any of:
///   - it is open (no ended_at) and started_at <= interval_end;
///   - its [started_at, ended_at] overlaps [interval_start, interval_end];
///   - it ended within `lookback_minutes` before interval_start.
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
        // Overlap test.
        if end >= interval_start && started <= interval_end {
            return Ok(true);
        }
        // Recent lookback test.
        let cutoff = interval_start - chrono::Duration::minutes(lookback_minutes);
        if end >= cutoff && end < interval_start {
            return Ok(true);
        }
        Ok(false)
    } else {
        // Open event: attach if it started before or during the interval.
        Ok(started <= interval_end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> DateTime<Utc> {
        parse_timestamp(s).unwrap()
    }

    fn ev(id: &str, t: &str, started: &str, ended: Option<&str>) -> Event {
        Event {
            event_id: id.into(),
            event_type: t.into(),
            started_at: started.into(),
            ended_at: ended.map(|s| s.into()),
            description: "test".into(),
            affected_components: vec![],
            operator: "op".into(),
        }
    }

    #[test]
    fn known_event_types() {
        for t in EVENT_TYPES {
            assert!(validate_event_type(t).is_ok());
        }
        assert!(validate_event_type("invented").is_err());
    }

    #[test]
    fn sort_is_deterministic() {
        let mut a = vec![
            ev("evt-3", "failure", "2026-05-15T09:00:00Z", None),
            ev("evt-1", "module_cleaning", "2026-05-15T08:00:00Z", Some("2026-05-15T09:00:00Z")),
            ev("evt-2", "scheduled_maintenance", "2026-05-15T08:00:00Z", Some("2026-05-15T08:30:00Z")),
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
        let e = ev(
            "x",
            "module_cleaning",
            "2026-05-15T10:00:00Z",
            Some("2026-05-15T10:20:00Z"),
        );
        assert!(should_attach(&e, start, end, 240).unwrap());
    }

    #[test]
    fn attach_recent_event_via_lookback() {
        let start = ts("2026-05-15T10:15:00Z");
        let end = ts("2026-05-15T10:30:00Z");
        let e = ev(
            "x",
            "module_cleaning",
            "2026-05-15T08:00:00Z",
            Some("2026-05-15T09:00:00Z"),
        );
        assert!(should_attach(&e, start, end, 240).unwrap());
    }

    #[test]
    fn skip_old_event_beyond_lookback() {
        let start = ts("2026-05-15T10:15:00Z");
        let end = ts("2026-05-15T10:30:00Z");
        let e = ev(
            "x",
            "module_cleaning",
            "2026-05-14T08:00:00Z",
            Some("2026-05-14T09:00:00Z"),
        );
        assert!(!should_attach(&e, start, end, 240).unwrap());
    }

    #[test]
    fn reject_missing_z_suffix() {
        assert!(parse_timestamp("2026-05-15T10:15:00").is_err());
        assert!(parse_timestamp("2026-05-15T10:15:00+00:00").is_err());
        assert!(parse_timestamp("2026-05-15T10:15:00Z").is_ok());
    }
}
