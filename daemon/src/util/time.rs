use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SECONDS_PER_DAY: u64 = 86_400;

/// When `deadline` comes, in Unix milliseconds.
pub fn unix_ms(deadline: Instant) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    let ahead = deadline.saturating_duration_since(Instant::now());
    u64::try_from((now + ahead).as_millis()).unwrap_or(u64::MAX)
}

/// `20261105T101500Z` for `time`, for file names.
pub fn utc_stamp(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let (year, month, day) = civil_date(seconds / SECONDS_PER_DAY);
    let second_of_day = seconds % SECONDS_PER_DAY;
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        second_of_day / 3600,
        second_of_day / 60 % 60,
        second_of_day % 60
    )
}

/// The proleptic Gregorian date `days` after 1970-01-01 (Howard Hinnant's
/// `civil_from_days`).
fn civil_date(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_stamps() {
        assert_eq!(utc_stamp(UNIX_EPOCH), "19700101T000000Z");
        let november = UNIX_EPOCH + Duration::from_secs(1_793_960_100);
        assert_eq!(utc_stamp(november), "20261106T101500Z");
        let leap_day = UNIX_EPOCH + Duration::from_secs(1_709_164_800);
        assert_eq!(utc_stamp(leap_day), "20240229T000000Z");
    }
}
