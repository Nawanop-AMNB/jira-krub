use chrono::{Datelike, NaiveDate, Weekday};
use std::collections::{BTreeMap, HashMap};

/// Which days need hours and how many: workdays, public holidays, and the
/// daily target (with per-year overrides). Pure data, built from config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkCalendar {
    /// Mon..Sun
    pub workdays: [bool; 7],
    /// date → holiday name
    pub holidays: BTreeMap<NaiveDate, String>,
    /// seconds per workday
    pub default_target: u64,
    /// year → seconds per workday
    pub year_target: HashMap<i32, u64>,
}

pub const MON_TO_FRI: [bool; 7] = [true, true, true, true, true, false, false];

impl WorkCalendar {
    /// Mon–Fri, no holidays, one target everywhere.
    pub fn simple(target_seconds: u64) -> Self {
        Self { workdays: MON_TO_FRI, holidays: BTreeMap::new(), default_target: target_seconds, year_target: HashMap::new() }
    }

    pub fn holiday(&self, d: NaiveDate) -> Option<&str> {
        self.holidays.get(&d).map(String::as_str)
    }

    /// A day that needs hours: enabled weekday and not a holiday.
    pub fn is_workday(&self, d: NaiveDate) -> bool {
        self.workdays[d.weekday().num_days_from_monday() as usize] && !self.holidays.contains_key(&d)
    }

    /// Why a day is off, for display. `None` on workdays.
    pub fn off_reason(&self, d: NaiveDate) -> Option<String> {
        if let Some(name) = self.holiday(d) {
            return Some(name.to_string());
        }
        (!self.workdays[d.weekday().num_days_from_monday() as usize]).then(|| weekday_name(d.weekday()).to_string())
    }

    /// Seconds expected on `d` (0 on off days).
    pub fn target_for(&self, d: NaiveDate) -> u64 {
        if !self.is_workday(d) {
            return 0;
        }
        self.year_target.get(&d.year()).copied().unwrap_or(self.default_target)
    }

    /// Sum of daily targets over an inclusive date range.
    pub fn target_between(&self, from: NaiveDate, to: NaiveDate) -> u64 {
        let mut total = 0;
        let mut d = from;
        while d <= to {
            total += self.target_for(d);
            d = d.succ_opt().unwrap();
        }
        total
    }
}

impl Default for WorkCalendar {
    fn default() -> Self {
        Self::simple(8 * 3600)
    }
}

fn weekday_name(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn workdays_and_holidays() {
        let mut c = WorkCalendar::simple(8 * 3600);
        c.holidays.insert(d("2026-09-16"), "Test day".into());
        assert!(c.is_workday(d("2026-09-15")));
        assert!(!c.is_workday(d("2026-09-16")), "holiday");
        assert!(!c.is_workday(d("2026-09-19")), "saturday");
        assert_eq!(c.off_reason(d("2026-09-16")).as_deref(), Some("Test day"));
        assert_eq!(c.off_reason(d("2026-09-19")).as_deref(), Some("Saturday"));
        assert_eq!(c.off_reason(d("2026-09-15")), None);
    }

    #[test]
    fn targets_with_year_override_and_week_sum() {
        let mut c = WorkCalendar::simple(8 * 3600);
        c.year_target.insert(2026, 7 * 3600);
        c.holidays.insert(d("2026-09-16"), "x".into());
        assert_eq!(c.target_for(d("2026-09-15")), 7 * 3600);
        assert_eq!(c.target_for(d("2025-09-15")), 8 * 3600, "no override in 2025");
        assert_eq!(c.target_for(d("2026-09-16")), 0, "holiday");
        // week of 14 Sep 2026: 5 workdays − 1 holiday = 4 × 7h
        assert_eq!(c.target_between(d("2026-09-14"), d("2026-09-20")), 4 * 7 * 3600);
        c.workdays[4] = false; // no Fridays
        assert_eq!(c.target_between(d("2026-09-14"), d("2026-09-20")), 3 * 7 * 3600);
    }
}
