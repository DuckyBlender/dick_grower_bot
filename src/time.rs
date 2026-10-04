use chrono::{Days, NaiveDateTime, Utc};

/// Timestamp format used by every TEXT datetime column (same as SQLite's `datetime('now')`).
const DB_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub fn now() -> NaiveDateTime {
    Utc::now().naive_utc()
}

pub fn parse(value: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, DB_FORMAT).ok()
}

pub fn format(time: NaiveDateTime) -> String {
    time.format(DB_FORMAT).to_string()
}

/// Discord relative timestamp, e.g. "in 5 minutes".
pub fn relative(time: NaiveDateTime) -> String {
    format!("<t:{}:R>", time.and_utc().timestamp())
}

pub fn next_utc_midnight() -> NaiveDateTime {
    let tomorrow = Utc::now().date_naive() + Days::new(1);
    tomorrow
        .and_hms_opt(0, 0, 0)
        .expect("midnight is a valid time")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_db_format() {
        let parsed = parse("2025-03-10 12:34:56").unwrap();
        assert_eq!(format(parsed), "2025-03-10 12:34:56");
        assert!(parse("garbage").is_none());
    }

    #[test]
    fn next_midnight_is_in_the_future() {
        let midnight = next_utc_midnight();
        assert!(midnight > now());
        assert_eq!(midnight.time(), chrono::NaiveTime::MIN);
    }
}
