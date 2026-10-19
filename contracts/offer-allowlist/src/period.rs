//! The period rule (ADR 0007): usage is counted per UTC calendar month, and
//! a month is named by `period = year * 12 + (month - 1)`, so `period / 12`
//! is the year and `period % 12` the zero-based month. Only this module
//! turns a ledger timestamp into a period; the sale contract keeps receipt
//! numbers and never handles a period, and the backend reads periods from
//! the allowlist's events and getters.
//!
//! The civil-date arithmetic is Howard Hinnant's `civil_from_days`, which is
//! exact for every non-negative day count. A ledger timestamp is seconds
//! since the Unix epoch, so day counts are never negative.

const SECONDS_PER_DAY: u64 = 86_400;

/// The period of the UTC calendar month `timestamp` falls in, or `None` when
/// `year * 12 + (month - 1)` does not fit a `u32`. Unreachable with a real
/// ledger time (that is year 357,913,941), defined so the function has a
/// contract for every input.
pub fn from_timestamp(timestamp: u64) -> Option<u32> {
    let (year, month) = civil_from_days(timestamp / SECONDS_PER_DAY);
    let period = year.checked_mul(12)?.checked_add(u64::from(month) - 1)?;
    u32::try_from(period).ok()
}

/// Year and month (1–12) of the day `days` after 1970-01-01.
fn civil_from_days(days: u64) -> (u64, u32) {
    // Shift the epoch to 0000-03-01 so each 400-year era starts on March 1
    // and the leap day is the last day of the era's year.
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097; // day of era, [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year, March-based
    let mp = (5 * doy + 2) / 153; // March-based month, [0, 11]
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month)
}

#[cfg(test)]
mod test {
    //! Pure arithmetic against known timestamps, including the `u32` bound.
    use super::from_timestamp;

    /// `year * 12 + (month - 1)`, the way a reader would compute it.
    fn period(year: u32, month: u32) -> Option<u32> {
        Some(year * 12 + (month - 1))
    }

    /// Days since 1970-01-01 of a civil date (Hinnant's `days_from_civil`),
    /// the inverse of the module's arithmetic, used to name test inputs.
    fn timestamp(year: u64, month: u64, day: u64, h: u64, m: u64, s: u64) -> u64 {
        let y = if month <= 2 { year - 1 } else { year };
        let era = y / 400;
        let yoe = y - era * 400;
        let mp = if month > 2 { month - 3 } else { month + 9 };
        let doy = (153 * mp + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        days * 86_400 + h * 3_600 + m * 60 + s
    }

    #[test]
    fn the_epoch_is_january_1970() {
        assert_eq!(from_timestamp(0), period(1970, 1));
    }

    #[test]
    fn the_last_second_of_1999_is_december_1999() {
        assert_eq!(
            from_timestamp(timestamp(1999, 12, 31, 23, 59, 59)),
            period(1999, 12)
        );
    }

    #[test]
    fn the_first_second_of_2000_is_january_2000() {
        assert_eq!(
            from_timestamp(timestamp(2000, 1, 1, 0, 0, 0)),
            period(2000, 1)
        );
        assert_eq!(from_timestamp(946_684_800), period(2000, 1));
    }

    #[test]
    fn a_leap_day_belongs_to_february() {
        assert_eq!(
            from_timestamp(timestamp(2024, 2, 29, 12, 0, 0)),
            period(2024, 2)
        );
        assert_eq!(from_timestamp(1_709_208_000), period(2024, 2));
    }

    #[test]
    fn the_spec_date_is_october_2026() {
        assert_eq!(
            from_timestamp(timestamp(2026, 10, 15, 0, 0, 0)),
            period(2026, 10)
        );
        assert_eq!(from_timestamp(1_792_022_400), period(2026, 10));
    }

    #[test]
    fn a_year_boundary_is_one_period_apart() {
        let december = from_timestamp(timestamp(2026, 12, 31, 23, 59, 59)).unwrap();
        let january = from_timestamp(timestamp(2027, 1, 1, 0, 0, 0)).unwrap();
        assert_eq!(december, 2026 * 12 + 11);
        assert_eq!(january, december + 1);
        assert_eq!(january, 2027 * 12);
    }

    #[test]
    fn year_and_month_can_be_read_back_from_the_period() {
        let p = from_timestamp(timestamp(2026, 10, 15, 0, 0, 0)).unwrap();
        assert_eq!(p / 12, 2026);
        assert_eq!(p % 12 + 1, 10);
    }

    #[test]
    fn the_largest_period_fits_and_the_next_second_does_not() {
        // u32::MAX = 4_294_967_295 = 357_913_941 * 12 + 3: year 357,913,941,
        // April. The last second of that April is the last timestamp whose
        // period fits.
        let last = timestamp(357_913_941, 4, 30, 23, 59, 59);
        assert_eq!(from_timestamp(last), Some(u32::MAX));
        assert_eq!(from_timestamp(last + 1), None);
        assert_eq!(from_timestamp(u64::MAX), None);
    }
}
