use crate::usage_history::consts::SECS_PER_DAY;
use crate::usage_history::structs::Day;

impl Day {
    pub fn new(year: i32, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    /// Days since 1970-01-01 (civil calendar).
    pub fn epoch_days(self) -> i64 {
        let m = i64::from(self.month);
        let y = i64::from(self.year) - i64::from(m <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = if m > 2 { m - 3 } else { m + 9 };
        let doy = (153 * mp + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    pub fn from_epoch_days(days: i64) -> Self {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        Self { year: year as i32, month: month as u8, day }
    }

    /// The local day of an instant, for a zone `offset_secs` east of UTC.
    pub fn from_epoch_secs(secs: i64, offset_secs: i32) -> Self {
        Self::from_epoch_days((secs + i64::from(offset_secs)).div_euclid(SECS_PER_DAY))
    }

    /// The instant (epoch seconds) this day starts, for a zone `offset_secs` east of UTC.
    pub fn start_secs(self, offset_secs: i32) -> i64 {
        self.epoch_days() * SECS_PER_DAY - i64::from(offset_secs)
    }

    pub fn plus_days(self, n: i64) -> Self {
        Self::from_epoch_days(self.epoch_days() + n)
    }

    pub fn minus_days(self, n: i64) -> Self {
        self.plus_days(-n)
    }
}
