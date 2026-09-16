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
    Save,
    Cancel,
    /// Cancel confirmed although there are unsaved edits.
    Discard,
}
