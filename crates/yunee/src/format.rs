//! Human-facing formatting: due dates in the local timezone, file sizes.
//!
//! Canvas sends ISO-8601 instants (usually with `Z`). Everything here renders
//! them in the machine's local time, the way a calendar app would.

use chrono::{DateTime, Datelike, Local, NaiveDateTime, Timelike};

/// Parse a Canvas timestamp into local time, tolerating a missing offset.
pub fn parse_local(iso: &str) -> Option<DateTime<Local>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(iso) {
        return Some(dt.with_timezone(&Local));
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(iso, fmt) {
            if let Some(dt) = naive.and_local_timezone(Local).single() {
                return Some(dt);
            }
        }
    }
    None
}

fn time_12(dt: &DateTime<Local>) -> String {
    let hour = dt.hour();
    let h12 = if hour % 12 == 0 { 12 } else { hour % 12 };
    let ampm = if hour < 12 { "AM" } else { "PM" };
    format!("{h12}:{:02} {ampm}", dt.minute())
}

/// A friendly due label: "Today 2:00 PM", "Tomorrow", "Fri", "Oct 24".
pub fn due_label(iso: &str) -> String {
    let Some(dt) = parse_local(iso) else {
        return iso.to_string();
    };
    let today = Local::now().date_naive();
    let days = (dt.date_naive() - today).num_days();
    match days {
        0 => format!("Today {}", time_12(&dt)),
        1 => format!("Tomorrow {}", time_12(&dt)),
        -1 => format!("Yesterday {}", time_12(&dt)),
        n if (-6..=6).contains(&n) => format!("{} {}", dt.format("%a"), time_12(&dt)),
        _ => format!("{} {}, {}", dt.format("%b"), dt.day(), time_12(&dt)),
    }
}

/// Whole days until the instant (negative if past).
pub fn days_until(iso: &str) -> Option<i64> {
    let dt = parse_local(iso)?;
    Some((dt.date_naive() - Local::now().date_naive()).num_days())
}

pub fn is_overdue(iso: &str) -> bool {
    parse_local(iso)
        .map(|dt| dt < Local::now())
        .unwrap_or(false)
}

/// "3.2 MB" and friends.
pub fn bytes_label(size: i64) -> String {
    let size = size.max(0) as f64;
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    if size >= GB {
        format!("{:.1} GB", size / GB)
    } else if size >= MB {
        format!("{:.1} MB", size / MB)
    } else if size >= KB {
        format!("{:.0} KB", size / KB)
    } else {
        format!("{size:.0} B")
    }
}

/// "Oct 5" for announcement headers.
pub fn short_date(iso: &str) -> String {
    parse_local(iso)
        .map(|dt| format!("{} {}", dt.format("%b"), dt.day()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(bytes_label(512), "512 B");
        assert_eq!(bytes_label(2048), "2 KB");
        assert_eq!(bytes_label(3 * 1024 * 1024), "3.0 MB");
    }

    #[test]
    fn relative_labels_are_stable_for_fixed_dates() {
        // Not asserting the local render (that depends on the machine), only
        // that parsing works and produces something non-empty and tag-free.
        let label = due_label("2026-10-20T18:30:00Z");
        assert!(!label.is_empty());
        assert!(!label.contains('T'));
    }
}
