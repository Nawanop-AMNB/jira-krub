//! Settings model: lenient holiday dates (R16) and the Year-tab row list.

use super::model::{HolidayRow, Model, YearField, normalise_date};
use crate::application::config::{Holiday, YearSettings};
use crate::application::{Config, Credentials};
use crate::domain::SiteUrl;
use chrono::NaiveDate;

fn iso(input: &str, year: i32) -> String {
    normalise_date(input, year).unwrap_or_else(|e| panic!("{input:?} should parse: {e}")).to_string()
}

fn holiday(y: i32, m: u32, d: u32, name: &str) -> Holiday {
    Holiday { date: NaiveDate::from_ymd_opt(y, m, d).unwrap(), name: name.to_string() }
}

fn config_with(year: i32, holidays: Vec<Holiday>) -> Config {
    let mut c = Config::with_defaults(Credentials {
        site: SiteUrl::parse("https://acme.atlassian.net").unwrap(),
        email: "me@example.com".into(),
        api_token: "tok".into(),
    });
    if !holidays.is_empty() {
        c.years.insert(year, YearSettings { hours_per_day: None, holidays });
    }
    c
}

// ---- normalise_date -------------------------------------------------------

#[test]
fn a_three_digit_date_is_read_as_month_then_day() {
    assert_eq!(iso("105", 2026), "2026-01-05");
    assert_eq!(iso("930", 2026), "2026-09-30");
}

#[test]
fn dots_separate_the_date_parts() {
    assert_eq!(iso("2026.12.05", 2026), "2026-12-05");
    assert_eq!(iso("12.5", 2026), "2026-12-05");
}

#[test]
fn spaces_separate_the_date_parts() {
    assert_eq!(iso("2026 12 05", 2026), "2026-12-05");
    assert_eq!(iso(" 12 5 ", 2026), "2026-12-05");
}

#[test]
fn a_leap_day_is_accepted_in_a_leap_year() {
    assert_eq!(iso("2028-02-29", 2028), "2028-02-29");
}

#[test]
fn a_leap_day_is_rejected_in_a_common_year() {
    let err = normalise_date("2026-02-29", 2026).expect_err("2026 is not a leap year");
    assert!(err.contains("not a valid date"), "{err}");
}

#[test]
fn a_two_digit_year_is_rejected_rather_than_read_as_year_26() {
    let err = normalise_date("26-12-05", 2026).expect_err("'26' must not pass as the year");
    assert!(err.contains("2026"), "{err}");
}

// ---- Model ----------------------------------------------------------------

#[test]
fn year_order_without_holidays_is_hours_add_save_cancel() {
    let cfg = config_with(2026, Vec::new());
    let m = Model::from_config(&cfg, 2026);
    assert_eq!(m.year_order(), vec![YearField::Hours, YearField::AddHoliday, YearField::Save, YearField::Cancel]);
}

#[test]
fn year_order_inserts_one_row_per_holiday() {
    let cfg = config_with(2026, vec![holiday(2026, 1, 1, "New Year"), holiday(2026, 4, 13, "Songkran")]);
    let m = Model::from_config(&cfg, 2026);
    assert_eq!(
        m.year_order(),
        vec![
            YearField::Hours,
            YearField::Holiday(0),
            YearField::Holiday(1),
            YearField::AddHoliday,
            YearField::Save,
            YearField::Cancel,
        ]
    );
}

#[test]
fn holiday_from_row_parses_an_iso_date_and_trims_the_name() {
    let row = HolidayRow { date: " 2026-01-01 ".into(), name: "  New Year  ".into() };
    let h = Model::holiday_from_row(&row).expect("parses");
    assert_eq!(h.date, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    assert_eq!(h.name, "New Year");
}

#[test]
fn holiday_from_row_rejects_anything_but_an_iso_date() {
    let row = HolidayRow { date: "12/05".into(), name: "Nope".into() };
    assert!(Model::holiday_from_row(&row).is_none(), "rows are stored already normalised");
}
