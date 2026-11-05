use std::time::{SystemTime, UNIX_EPOCH};

const SECONDS_PER_DAY: u64 = 86_400;
const SECONDS_PER_HOUR: u64 = 3_600;
const SECONDS_PER_MINUTE: u64 = 60;
/// Days from 0000-03-01, where the civil calendar below starts its eras, to 1970-01-01.
const EPOCH_DAYS_FROM_ERA_START: i64 = 719_468;
const DAYS_PER_ERA: i64 = 146_097;
const YEARS_PER_ERA: i64 = 400;

/// A UTC time to the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UtcTime {
    year: i64,
    month: u64,
    day: u64,
    hour: u64,
    minute: u64,
    second: u64,
}

impl UtcTime {
    pub fn now() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since_epoch| since_epoch.as_secs());
        Self::from_unix(seconds)
    }

    /// Converts with Howard Hinnant's `civil_from_days`.
    pub fn from_unix(seconds: u64) -> Self {
        let days = (seconds / SECONDS_PER_DAY) as i64 + EPOCH_DAYS_FROM_ERA_START;
        let era = days.div_euclid(DAYS_PER_ERA);
        let day_of_era = days.rem_euclid(DAYS_PER_ERA);
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let march_month = (5 * day_of_year + 2) / 153;
        let month = if march_month < 10 {
            march_month + 3
        } else {
            march_month - 9
        };
        let seconds_of_day = seconds % SECONDS_PER_DAY;
        Self {
            year: year_of_era + era * YEARS_PER_ERA + i64::from(month <= 2),
            month: month as u64,
            day: (day_of_year - (153 * march_month + 2) / 5 + 1) as u64,
            hour: seconds_of_day / SECONDS_PER_HOUR,
            minute: seconds_of_day % SECONDS_PER_HOUR / SECONDS_PER_MINUTE,
            second: seconds_of_day % SECONDS_PER_MINUTE,
        }
    }

    /// `20261105T093400Z`: a file name that sorts by time.
    pub fn compact(&self) -> String {
        let Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        } = self;
        format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
    }

    /// `2026-11-05T09:34:00Z`.
    pub fn iso8601(&self) -> String {
        let Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        } = self;
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_times() {
        let time = UtcTime::from_unix(1_793_871_240);
        assert_eq!(time.compact(), "20261105T093400Z");
        assert_eq!(time.iso8601(), "2026-11-05T09:34:00Z");
        assert_eq!(
            UtcTime::from_unix(951_868_799).iso8601(),
            "2000-02-29T23:59:59Z"
        );
        assert_eq!(UtcTime::from_unix(0).iso8601(), "1970-01-01T00:00:00Z");
    }
}
