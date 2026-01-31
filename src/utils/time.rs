//! Time utilities

use chrono::{DateTime, Duration, Utc};

/// Get current UTC time
pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

/// Parse a datetime string in ISO 8601 format
pub fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

/// Format a duration as a human-readable string
pub fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.num_seconds();

    if total_seconds < 0 {
        return "0s".to_string();
    }

    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    let mut parts = Vec::new();

    if days > 0 {
        parts.push(format!("{}d", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if minutes > 0 {
        parts.push(format!("{}m", minutes));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{}s", seconds));
    }

    parts.join(" ")
}

/// Format milliseconds as a human-readable string
pub fn format_milliseconds(ms: i64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60000 {
        format!("{:.2}s", ms as f64 / 1000.0)
    } else {
        let seconds = ms / 1000;
        let minutes = seconds / 60;
        let remaining_seconds = seconds % 60;
        format!("{}m {}s", minutes, remaining_seconds)
    }
}

/// Calculate time until a future datetime
pub fn time_until(target: DateTime<Utc>) -> Option<Duration> {
    let now = now_utc();
    if target > now {
        Some(target - now)
    } else {
        None
    }
}

/// Calculate time since a past datetime
pub fn time_since(target: DateTime<Utc>) -> Option<Duration> {
    let now = now_utc();
    if target < now {
        Some(now - target)
    } else {
        None
    }
}

/// Check if a datetime is in the past
pub fn is_past(dt: DateTime<Utc>) -> bool {
    dt < now_utc()
}

/// Check if a datetime is in the future
pub fn is_future(dt: DateTime<Utc>) -> bool {
    dt > now_utc()
}

/// Check if current time is between two datetimes
pub fn is_between(start: DateTime<Utc>, end: DateTime<Utc>) -> bool {
    let now = now_utc();
    now >= start && now <= end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::seconds(30)), "30s");
        assert_eq!(format_duration(Duration::seconds(90)), "1m 30s");
        assert_eq!(format_duration(Duration::seconds(3661)), "1h 1m 1s");
        assert_eq!(format_duration(Duration::seconds(86400)), "1d");
    }

    #[test]
    fn test_format_milliseconds() {
        assert_eq!(format_milliseconds(500), "500ms");
        assert_eq!(format_milliseconds(1500), "1.50s");
        assert_eq!(format_milliseconds(65000), "1m 5s");
    }

    #[test]
    fn test_parse_datetime() {
        let dt = parse_datetime("2024-01-15T12:00:00Z");
        assert!(dt.is_some());

        let invalid = parse_datetime("not a date");
        assert!(invalid.is_none());
    }
}
