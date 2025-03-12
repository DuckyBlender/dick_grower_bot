use chrono::{Duration, NaiveDateTime, Utc};

// Combined function that checks for a new day and returns time until next reset
pub fn check_utc_day_reset(last_time: &NaiveDateTime) -> Duration {
    let now = Utc::now().naive_utc();

    // Calculate time until next reset
    let tomorrow = (Utc::now() + Duration::days(1))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap();

    let time_until_reset = tomorrow - now;
    let has_passed = now.date() > last_time.date();
    if has_passed {
        Duration::zero()
    } else {
        time_until_reset
    }
}

// Combined function that checks if 30 minutes passed and returns time remaining
pub fn check_30_minutes(last_time: &NaiveDateTime) -> Duration {
    let now = Utc::now().naive_utc();
    let duration = now - *last_time;
    let threshold = Duration::minutes(30);

    let has_passed = duration >= threshold;
    if has_passed {
        Duration::zero()
    } else {
        threshold - duration
    }
}
