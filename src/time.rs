use chrono::{Datelike, Duration, NaiveDateTime, Utc};

// Combined function that checks for a new day and returns time until next reset
pub fn check_utc_day_reset(last_time: &NaiveDateTime) -> (bool, Duration) {
    let now = Utc::now().naive_utc();
    
    // Get date parts for new day check
    let now_date = (now.year(), now.month(), now.day());
    let last_date = (last_time.year(), last_time.month(), last_time.day());
    
    // Calculate time until next reset
    let tomorrow = (Utc::now() + Duration::days(1))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    
    let time_until_reset = tomorrow - now;
    
    // Return both results as a tuple
    (now_date != last_date, time_until_reset)
}

// Combined function that checks if 30 minutes passed and returns time remaining
pub fn check_30_minutes(last_time: &NaiveDateTime) -> (bool, Duration) {
    let now = Utc::now().naive_utc();
    let duration = now - *last_time;
    let threshold = Duration::minutes(30);
    
    let has_passed = duration >= threshold;
    let time_remaining = if has_passed {
        Duration::zero()
    } else {
        threshold - duration
    };
    
    (has_passed, time_remaining)
}