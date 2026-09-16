use crate::application::Config;
use crate::application::config::{Holiday, YearSettings};
use crate::domain::duration;
use crate::tui::widgets::TextInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Global,
    Year,
}

/// Focusable rows on the Global tab, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalField {
    Hours,
    Workdays,
    Start,
    QuickStage,
    Lookback,
    AutoWatch,
    Connection,
    Save,
    Cancel,
}

impl GlobalField {
    pub const ORDER: [GlobalField; 9] = [
        GlobalField::Hours,
        GlobalField::Workdays,
        GlobalField::Start,
        GlobalField::QuickStage,
        GlobalField::Lookback,
        GlobalField::AutoWatch,
        GlobalField::Connection,
        GlobalField::Save,
        GlobalField::Cancel,
    ];
    pub fn is_text(self) -> bool {
        matches!(self, GlobalField::Hours | GlobalField::Start | GlobalField::QuickStage | GlobalField::Lookback)
    }
}

/// Focusable rows on the Year tab: hours, then one row per holiday, then
/// the "+ add" row, then buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YearField {
    Hours,
    Holiday(usize),
    AddHoliday,
    Save,
    Cancel,
}

/// A holiday row being edited: date, then name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HolidayEdit {
    pub index: usize,
    pub date: TextInput,
    pub name: TextInput,
    pub on_name: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HolidayRow {
    pub date: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearDraft {
    pub year: i32,
    pub hours: TextInput,
    pub holidays: Vec<HolidayRow>,
}

impl YearDraft {
    pub fn from_settings(year: i32, ys: &YearSettings) -> Self {
        Self {
            year,
            hours: TextInput::with(ys.hours_per_day.map(|h| h.to_string()).unwrap_or_default()),
            holidays: ys.holidays.iter().map(|h| HolidayRow { date: h.date.format("%Y-%m-%d").to_string(), name: h.name.clone() }).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub tab: Tab,
    // ---- global draft ----
    pub hours: TextInput,
    pub workdays: [bool; 7],
    pub workday_cursor: usize,
    pub start: TextInput,
    pub quick_stage: TextInput,
    pub lookback: TextInput,
    pub auto_watch: bool,
    pub g_focus: GlobalField,
    // ---- year drafts (one per year touched) ----
    pub years: Vec<YearDraft>,
    pub year: i32,
    pub y_focus: YearField,
    pub holiday_edit: Option<HolidayEdit>,
    // ---- state ----
    pub dirty: bool,
    pub error: Option<String>,
    /// Lookback at open, to know whether a save needs a re-sync.
    pub initial_lookback: u32,
    /// EDIT mode: the focused text field (or the workday chips) is open.
    pub editing: bool,
    /// Value to restore on Esc.
    pub backup: String,
    pub backup_workdays: [bool; 7],
}

impl Model {
    pub fn from_config(cfg: &Config, year: i32) -> Self {
        let g = &cfg.global;
        let mut m = Self {
            tab: Tab::Global,
            hours: TextInput::with(g.hours_per_day.to_string()),
            workdays: g.workdays,
            workday_cursor: 0,
            start: TextInput::with(g.default_start.to_string()),
            quick_stage: TextInput::with(duration::format(g.quick_stage_seconds).replace(' ', "")),
            lookback: TextInput::with(g.lookback_weeks.to_string()),
            auto_watch: g.auto_watch,
            g_focus: GlobalField::Hours,
            years: Vec::new(),
            year,
            y_focus: YearField::Hours,
            holiday_edit: None,
            dirty: false,
            error: None,
            initial_lookback: g.lookback_weeks,
            editing: false,
            backup: String::new(),
            backup_workdays: g.workdays,
        };
        m.years.push(YearDraft::from_settings(year, &cfg.year(year)));
        m
    }

    pub fn year_draft(&mut self, cfg: &Config) -> &mut YearDraft {
        let y = self.year;
        if !self.years.iter().any(|d| d.year == y) {
            self.years.push(YearDraft::from_settings(y, &cfg.year(y)));
        }
        self.years.iter_mut().find(|d| d.year == y).unwrap()
    }

    pub fn current_year(&self) -> Option<&YearDraft> {
        self.years.iter().find(|d| d.year == self.year)
    }

    pub fn global_text_mut(&mut self) -> Option<&mut TextInput> {
        match self.g_focus {
            GlobalField::Hours => Some(&mut self.hours),
            GlobalField::Start => Some(&mut self.start),
            GlobalField::QuickStage => Some(&mut self.quick_stage),
            GlobalField::Lookback => Some(&mut self.lookback),
            _ => None,
        }
    }

    /// Year-tab rows in order, for ↑/↓.
    pub fn year_order(&self) -> Vec<YearField> {
        let n = self.current_year().map(|d| d.holidays.len()).unwrap_or(0);
        let mut v = vec![YearField::Hours];
        v.extend((0..n).map(YearField::Holiday));
        v.push(YearField::AddHoliday);
        v.push(YearField::Save);
        v.push(YearField::Cancel);
        v
    }

    pub fn holiday_from_row(row: &HolidayRow) -> Option<Holiday> {
        let date = chrono::NaiveDate::parse_from_str(row.date.trim(), "%Y-%m-%d").ok()?;
        Some(Holiday { date, name: row.name.trim().to_string() })
    }
}

/// Lenient date entry, normalised to `YYYY-MM-DD` for the given year:
/// `2026-12-5`, `2026/12/05`, `12-5`, `12/05`, `1205`, `20261205`.
/// Month/day are checked; a leading year, if given, must match `year`.
pub fn normalise_date(input: &str, year: i32) -> Result<chrono::NaiveDate, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("date is required".into());
    }
    let parts: Vec<&str> = s.split(['-', '/', '.', ' ']).filter(|p| !p.is_empty()).collect();
    let (y, m, d): (i32, u32, u32) = match parts.as_slice() {
        [y, m, d] => (y.parse().map_err(|_| "bad year")?, m.parse().map_err(|_| "bad month")?, d.parse().map_err(|_| "bad day")?),
        [m, d] => (year, m.parse().map_err(|_| "bad month")?, d.parse().map_err(|_| "bad day")?),
        [digits] if digits.chars().all(|c| c.is_ascii_digit()) => match digits.len() {
            8 => (digits[..4].parse().unwrap(), digits[4..6].parse().unwrap(), digits[6..].parse().unwrap()),
            4 => (year, digits[..2].parse().unwrap(), digits[2..].parse().unwrap()),
            3 => (year, digits[..1].parse().unwrap(), digits[1..].parse().unwrap()),
            _ => return Err("use YYYY-MM-DD or MM-DD".into()),
        },
        _ => return Err("use YYYY-MM-DD or MM-DD".into()),
    };
    if y != year {
        return Err(format!("date must be in {year}"));
    }
    chrono::NaiveDate::from_ymd_opt(y, m, d).ok_or_else(|| format!("{m:02}-{d:02} is not a valid date"))
}

#[cfg(test)]
mod tests {
    use super::normalise_date;
    #[test]
    fn lenient_dates() {
        let ok = |s: &str| normalise_date(s, 2026).unwrap().to_string();
        assert_eq!(ok("2026-12-5"), "2026-12-05");
        assert_eq!(ok("2026/12/05"), "2026-12-05");
        assert_eq!(ok("12-5"), "2026-12-05");
        assert_eq!(ok("12/05"), "2026-12-05");
        assert_eq!(ok("1205"), "2026-12-05");
        assert_eq!(ok("20261205"), "2026-12-05");
        assert_eq!(ok("1-1"), "2026-01-01");
        assert!(normalise_date("2025-12-05", 2026).is_err(), "wrong year");
        assert!(normalise_date("13-01", 2026).is_err(), "bad month");
        assert!(normalise_date("02-30", 2026).is_err(), "bad day");
        assert!(normalise_date("", 2026).is_err());
        assert!(normalise_date("abc", 2026).is_err());
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    SwitchTab,
    SetTab(Tab),
    Up,
    Down,
    /// Click on a specific row.
    FocusGlobal(GlobalField),
    FocusYear(YearField),
    Left,
    Right,
    Char(char),
    Paste(String),
    Backspace,
    Delete,
    Home,
    End,
    /// Enter: toggle / open / activate depending on focus.
    Activate,
    PrevYear,
    NextYear,
    RemoveHoliday,
    /// Esc inside a holiday row: drop that edit only.
    CancelHolidayEdit,
    /// Commit the open field, back to NAV.
    Commit,
    /// Esc in EDIT: restore the field, back to NAV.
    Revert,
    Save,
    Cancel,
    /// Cancel confirmed although there are unsaved edits.
    Discard,
}
