use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};

pub fn active(timestamp: DateTime<Utc>) -> bool {
    match timestamp.weekday() {
        Weekday::Fri => timestamp.hour() >= 22,
        Weekday::Sat => true,
        Weekday::Sun => timestamp.hour() < 22,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::active;

    #[test]
    fn weekend_ban_matches_rulebook_window() {
        let friday = Utc.with_ymd_and_hms(2026, 1, 2, 22, 0, 0).unwrap();
        let sunday = Utc.with_ymd_and_hms(2026, 1, 4, 21, 59, 0).unwrap();
        let sunday_clear = Utc.with_ymd_and_hms(2026, 1, 4, 22, 0, 0).unwrap();
        assert!(active(friday));
        assert!(active(sunday));
        assert!(!active(sunday_clear));
    }
}
