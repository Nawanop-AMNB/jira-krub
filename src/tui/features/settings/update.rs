use super::model::{Action, GlobalField, HolidayEdit, HolidayRow, Model, Tab, YearField, normalise_date};
use crate::application::Config;
use crate::application::config::{GlobalSettings, YearSettings};
use crate::domain::{StartTime, duration};
use crate::tui::action::Action as Global;
use crate::tui::app::{App, Screen};
use crate::tui::widgets::TextInput;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn keys(m: &Model, key: &KeyEvent) -> Option<Global> {
    use Action::*;
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if key.code == KeyCode::Char('s') && ctrl {
        return Some(Global::Settings(Save));
    }
    let a = if m.holiday_edit.is_some() {
        // holiday row walk: date → name
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => SwitchTab,
            KeyCode::Esc => CancelHolidayEdit,
            KeyCode::Enter => Activate,
            KeyCode::Up => Up,
            KeyCode::Down => Down,
            KeyCode::Left => Left,
            KeyCode::Right => Right,
            KeyCode::Home => Home,
            KeyCode::End => End,
            KeyCode::Backspace => Backspace,
            KeyCode::Delete => Delete,
            KeyCode::Char(c) if !ctrl => Char(c),
            _ => return None,
        }
    } else if m.editing {
        // one text field (or the workday chips) is open
        let chips = m.tab == Tab::Global && m.g_focus == GlobalField::Workdays;
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => SwitchTab,
            KeyCode::Esc => Revert,
            KeyCode::Enter => Commit,
            // chips: Space toggles, Enter commits, Esc reverts
            KeyCode::Char(' ') if chips => Activate,
            KeyCode::Up => Up,
            KeyCode::Down => Down,
            KeyCode::Left => Left,
            KeyCode::Right => Right,
            KeyCode::Home => Home,
            KeyCode::End => End,
            KeyCode::Backspace => Backspace,
            KeyCode::Delete => Delete,
            KeyCode::Char(c) if !ctrl => Char(c),
            _ => return None,
        }
    } else {
        // NAV: letters are hotkeys, arrows navigate
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => SwitchTab,
            KeyCode::Esc => Cancel,
            KeyCode::Enter | KeyCode::Char(' ') => Activate,
            KeyCode::Up => Up,
            KeyCode::Down => Down,
            KeyCode::Left | KeyCode::Char('[') if m.tab == Tab::Year => PrevYear,
            KeyCode::Right | KeyCode::Char(']') if m.tab == Tab::Year => NextYear,
            KeyCode::Backspace | KeyCode::Delete if m.tab == Tab::Year => RemoveHoliday,
            _ => return None,
        }
    };
    Some(Global::Settings(a))
}

/// Open the focused field for typing (text fields and the workday chips).
fn open_field(app: &mut App) {
    let cfg = app.config.clone();
    let Some(m) = model(app) else { return };
    match m.tab {
        Tab::Global => {
            if m.g_focus == GlobalField::Workdays {
                m.backup_workdays = m.workdays;
                m.editing = true;
            } else if m.g_focus.is_text() {
                m.backup = m.global_text_mut().map(|t| t.text().to_string()).unwrap_or_default();
                if let Some(t) = m.global_text_mut() {
                    t.end();
                }
                m.editing = true;
            }
        }
        Tab::Year => {
            if m.y_focus == YearField::Hours
                && let Some(cfg) = &cfg
            {
                let backup = {
                    let d = m.year_draft(cfg);
                    d.hours.end();
                    d.hours.text().to_string()
                };
                m.backup = backup;
                m.editing = true;
            }
        }
    }
    if m.editing {
        m.error = None;
    }
}

/// Close the open field, keeping its value (validation happens on Save).
fn commit_field(m: &mut Model) {
    if m.editing {
        m.editing = false;
        m.dirty = true;
    }
}

fn revert_field(app: &mut App) {
    let cfg = app.config.clone();
    let Some(m) = model(app) else { return };
    if !m.editing {
        return;
    }
    let backup = m.backup.clone();
    match m.tab {
        Tab::Global if m.g_focus == GlobalField::Workdays => m.workdays = m.backup_workdays,
        Tab::Global => {
            if let Some(t) = m.global_text_mut() {
                t.set(backup);
            }
        }
        Tab::Year => {
            if let Some(cfg) = &cfg {
                m.year_draft(cfg).hours.set(backup);
            }
        }
    }
    m.editing = false;
}

fn model(app: &mut App) -> Option<&mut Model> {
    match &mut app.screen {
        Screen::Settings(m) => Some(m),
        _ => None,
    }
}

// ---- validation / save --------------------------------------------------------

fn build_config(app: &App, m: &Model) -> Result<Config, (String, Option<GlobalField>)> {
    let base = app.config.clone().ok_or_else(|| ("not connected".to_string(), None))?;
    let hours: u32 = m.hours.text().trim().parse().ok().filter(|h| (1..=24).contains(h)).ok_or_else(|| ("target hours must be 1–24".to_string(), Some(GlobalField::Hours)))?;
    if !m.workdays.iter().any(|w| *w) {
        return Err(("pick at least one workday".into(), Some(GlobalField::Workdays)));
    }
    let start = StartTime::parse(m.start.text()).map_err(|e| (format!("start: {e}"), Some(GlobalField::Start)))?;
    let quick = duration::parse(m.quick_stage.text()).map_err(|e| (format!("quick-stage: {e}"), Some(GlobalField::QuickStage)))?;
    let lookback: u32 = m.lookback.text().trim().parse().ok().filter(|w| (1..=52).contains(w)).ok_or_else(|| ("lookback must be 1–52 weeks".to_string(), Some(GlobalField::Lookback)))?;

    let mut cfg = base;
    cfg.global = GlobalSettings { hours_per_day: hours, workdays: m.workdays, default_start: start, quick_stage_seconds: quick, lookback_weeks: lookback, auto_watch: m.auto_watch };
    for d in &m.years {
        let hours_per_day = if d.hours.is_empty() {
            None
        } else {
            Some(d.hours.text().trim().parse::<u32>().ok().filter(|h| (1..=24).contains(h)).ok_or_else(|| (format!("{}: target hours must be 1–24 or blank", d.year), None))?)
        };
        let mut holidays = Vec::new();
        for row in &d.holidays {
            let h = Model::holiday_from_row(row).ok_or_else(|| (format!("{}: bad holiday date '{}' (YYYY-MM-DD)", d.year, row.date), None))?;
            if h.date.format("%Y").to_string() != d.year.to_string() {
                return Err((format!("{}: holiday {} is not in that year", d.year, row.date), None));
            }
            holidays.push(h);
        }
        holidays.sort_by_key(|h| h.date);
        holidays.dedup_by_key(|h| h.date);
        let ys = YearSettings { hours_per_day, holidays };
        if ys.is_empty() {
            cfg.years.remove(&d.year);
        } else {
            cfg.years.insert(d.year, ys);
        }
    }
    Ok(cfg)
}

fn save(app: &mut App) {
    // finish an open holiday edit first
    if !commit_holiday(app) {
        return;
    }
    let Screen::Settings(m) = &app.screen else { return };
    let (cfg, initial_lookback) = match build_config(app, m) {
        Ok(c) => (c, m.initial_lookback),
        Err((msg, field)) => {
            let m = model(app).unwrap();
            m.error = Some(msg);
            if let Some(f) = field {
                m.tab = Tab::Global;
                m.g_focus = f;
            }
            return;
        }
    };
    if let Err(e) = app.deps.config_store.save(&cfg) {
        app.set_error(format!("could not save settings: {e}"));
        return;
    }
    let resync = cfg.global.lookback_weeks != initial_lookback;
    app.config = Some(cfg);
    app.set_main_tab(app.main_tab);
    app.set_status("settings saved");
    if resync {
        app.remote.window = None;
        app.start_sync();
    }
}

fn cancel(app: &mut App) {
    let dirty = model(app).is_some_and(|m| m.dirty);
    if dirty {
        app.confirm("discard unsaved settings? [y/n]", Global::Settings(Action::Discard));
    } else {
        app.set_main_tab(app.main_tab);
    }
}

// ---- holiday rows -------------------------------------------------------------

fn begin_holiday_edit(app: &mut App, index: usize) {
    let Some(m) = model(app) else { return };
    let Some(d) = m.current_year() else { return };
    let row = d.holidays.get(index).cloned().unwrap_or(HolidayRow { date: String::new(), name: String::new() });
    // Always walk date → name, even when editing an existing row.
    m.holiday_edit = Some(HolidayEdit { index, date: TextInput::with(row.date), name: TextInput::with(row.name), on_name: false, error: None });
    m.y_focus = YearField::Holiday(index);
}

/// Returns false if the open edit could not be committed (stays open with error).
fn commit_holiday(app: &mut App) -> bool {
    let cfg = app.config.clone();
    let Some(m) = model(app) else { return true };
    let Some(ed) = m.holiday_edit.clone() else { return true };
    let mut row = HolidayRow { date: ed.date.text().trim().to_string(), name: ed.name.text().trim().to_string() };
    if row.date.is_empty() && row.name.is_empty() {
        // empty new row → just drop it
        m.holiday_edit = None;
        let Some(cfg) = cfg else { return true };
        let d = m.year_draft(&cfg);
        if ed.index < d.holidays.len() && d.holidays[ed.index].date.is_empty() {
            d.holidays.remove(ed.index);
        }
        return true;
    }
    match normalise_date(&row.date, m.year) {
        Ok(date) => row.date = date.format("%Y-%m-%d").to_string(),
        Err(e) => {
            if let Some(ed) = &mut m.holiday_edit {
                ed.error = Some(e);
                ed.on_name = false;
            }
            return false;
        }
    }
    m.holiday_edit = None;
    m.dirty = true;
    let Some(cfg) = cfg else { return true };
    let d = m.year_draft(&cfg);
    if ed.index < d.holidays.len() {
        d.holidays[ed.index] = row;
    } else {
        d.holidays.push(row);
    }
    d.holidays.sort_by(|a, b| a.date.cmp(&b.date));
    true
}

// ---- update ---------------------------------------------------------------------

pub fn update(app: &mut App, action: Action) {
    use Action::*;
    let cfg = app.config.clone();
    match action {
        Save => save(app),
        Cancel => cancel(app),
        Discard => app.set_main_tab(app.main_tab),
        Commit => {
            if let Some(m) = model(app) {
                commit_field(m);
            }
        }
        Revert => revert_field(app),
        SwitchTab => {
            if !commit_holiday(app) {
                return;
            }
            let Some(m) = model(app) else { return };
            commit_field(m);
            m.tab = if m.tab == Tab::Global { Tab::Year } else { Tab::Global };
            m.error = None;
        }
        SetTab(t) => {
            if !commit_holiday(app) {
                return;
            }
            if let Some(m) = model(app) {
                commit_field(m);
                m.tab = t;
                m.error = None;
            }
        }
        // click = focus and open (click is Enter)
        FocusGlobal(f) => {
            if let Some(m) = model(app) {
                commit_field(m);
                m.tab = Tab::Global;
                m.g_focus = f;
            }
            open_field(app);
        }
        FocusYear(f) => {
            if !commit_holiday(app) {
                return;
            }
            if let Some(m) = model(app) {
                commit_field(m);
                m.tab = Tab::Year;
                m.y_focus = f;
            }
            if f == YearField::Hours {
                open_field(app);
            }
        }
        PrevYear | NextYear => {
            if !commit_holiday(app) {
                return;
            }
            let Some(cfg) = cfg else { return };
            let Some(m) = model(app) else { return };
            commit_field(m);
            m.year += if matches!(action, PrevYear) { -1 } else { 1 };
            m.year_draft(&cfg);
            // keep the cursor where it was so repeated ←/→ keep stepping years
            let order = m.year_order();
            if !order.contains(&m.y_focus) {
                m.y_focus = YearField::AddHoliday;
            }
        }
        Up | Down => {
            let delta: i32 = if matches!(action, Up) { -1 } else { 1 };
            if !commit_holiday(app) {
                return;
            }
            let Some(m) = model(app) else { return };
            commit_field(m);
            match m.tab {
                Tab::Global => {
                    let i = GlobalField::ORDER.iter().position(|f| *f == m.g_focus).unwrap_or(0) as i32;
                    let n = GlobalField::ORDER.len() as i32;
                    m.g_focus = GlobalField::ORDER[(i + delta).rem_euclid(n) as usize];
                }
                Tab::Year => {
                    let order = m.year_order();
                    let i = order.iter().position(|f| *f == m.y_focus).unwrap_or(0) as i32;
                    let n = order.len() as i32;
                    m.y_focus = order[(i + delta).rem_euclid(n) as usize];
                }
            }
        }
        Left | Right => {
            let delta: i32 = if matches!(action, Left) { -1 } else { 1 };
            let Some(m) = model(app) else { return };
            match m.tab {
                Tab::Global if m.g_focus == GlobalField::Workdays => {
                    if m.editing {
                        m.workday_cursor = (m.workday_cursor as i32 + delta).rem_euclid(7) as usize;
                    }
                }
                Tab::Global => {
                    if let Some(t) = m.global_text_mut() {
                        if delta < 0 { t.left() } else { t.right() }
                    }
                }
                Tab::Year => {
                    if let Some(ed) = &mut m.holiday_edit {
                        let t = if ed.on_name { &mut ed.name } else { &mut ed.date };
                        if delta < 0 { t.left() } else { t.right() }
                    } else if m.y_focus == YearField::Hours
                        && let Some(cfg) = &cfg
                    {
                        let d = m.year_draft(cfg);
                        if delta < 0 { d.hours.left() } else { d.hours.right() }
                    }
                }
            }
        }
        Home | End => {
            let home = matches!(action, Home);
            let Some(m) = model(app) else { return };
            let target = match m.tab {
                Tab::Global => m.global_text_mut(),
                Tab::Year => match &mut m.holiday_edit {
                    Some(ed) => Some(if ed.on_name { &mut ed.name } else { &mut ed.date }),
                    None => None,
                },
            };
            if let Some(t) = target {
                if home { t.home() } else { t.end() }
            }
        }
        Activate => {
            let Some(m) = model(app) else { return };
            match m.tab {
                Tab::Global => match m.g_focus {
                    GlobalField::Workdays => {
                        if m.editing {
                            m.workdays[m.workday_cursor] = !m.workdays[m.workday_cursor];
                            m.dirty = true;
                        } else {
                            open_field(app);
                        }
                    }
                    GlobalField::AutoWatch => {
                        m.auto_watch = !m.auto_watch;
                        m.dirty = true;
                    }
                    GlobalField::Connection => {
                        let mut c = crate::tui::features::connect::Model::new(app.config.as_ref().map(|c| &c.credentials), None);
                        c.from_settings = true;
                        app.overlay = None;
                        app.screen = Screen::Connect(c);
                    }
                    GlobalField::Save => save(app),
                    GlobalField::Cancel => cancel(app),
                    _ => {
                        if m.editing {
                            commit_field(m);
                        } else {
                            open_field(app);
                        }
                    }
                },
                Tab::Year => {
                    if let Some(ed) = &mut m.holiday_edit {
                        if ed.on_name {
                            commit_holiday(app);
                        } else {
                            // date typed → normalise, then go to name
                            match normalise_date(ed.date.text(), m.year) {
                                Ok(date) => {
                                    ed.date.set(date.format("%Y-%m-%d").to_string());
                                    ed.on_name = true;
                                    ed.error = None;
                                }
                                Err(e) => ed.error = Some(e),
                            }
                        }
                        return;
                    }
                    match m.y_focus {
                        YearField::Hours => {
                            if m.editing {
                                commit_field(m);
                            } else {
                                open_field(app);
                            }
                        }
                        YearField::Holiday(i) => begin_holiday_edit(app, i),
                        YearField::AddHoliday => {
                            let Some(cfg) = cfg else { return };
                            let d = m.year_draft(&cfg);
                            let idx = d.holidays.len();
                            begin_holiday_edit(app, idx);
                        }
                        YearField::Save => save(app),
                        YearField::Cancel => cancel(app),
                    }
                }
            }
        }
        CancelHolidayEdit => {
            let Some(m) = model(app) else { return };
            let Some(ed) = m.holiday_edit.take() else { return };
            let n = m.current_year().map(|d| d.holidays.len()).unwrap_or(0);
            m.y_focus = if ed.index < n { YearField::Holiday(ed.index) } else { YearField::AddHoliday };
        }
        RemoveHoliday => {
            let Some(cfg) = cfg else { return };
            let Some(m) = model(app) else { return };
            if m.tab != Tab::Year || m.holiday_edit.is_some() {
                return;
            }
            if let YearField::Holiday(i) = m.y_focus {
                let d = m.year_draft(&cfg);
                if i < d.holidays.len() {
                    d.holidays.remove(i);
                    m.dirty = true;
                    let n = m.current_year().map(|d| d.holidays.len()).unwrap_or(0);
                    m.y_focus = if n == 0 { YearField::AddHoliday } else { YearField::Holiday(i.min(n - 1)) };
                }
            }
        }
        Char(c) => {
            let Some(m) = model(app) else { return };
            m.error = None;
            match m.tab {
                Tab::Global => {
                    if let Some(t) = m.global_text_mut() {
                        t.insert(c);
                        m.dirty = true;
                    }
                }
                Tab::Year => {
                    if let Some(ed) = &mut m.holiday_edit {
                        if ed.on_name { ed.name.insert(c) } else { ed.date.insert(c) }
                        ed.error = None;
                    } else if m.y_focus == YearField::Hours
                        && let Some(cfg) = &cfg
                    {
                        m.year_draft(cfg).hours.insert(c);
                        m.dirty = true;
                    }
                }
            }
        }
        Paste(s) => {
            let Some(m) = model(app) else { return };
            match m.tab {
                Tab::Global => {
                    if let Some(t) = m.global_text_mut() {
                        t.insert_str(s.trim());
                        m.dirty = true;
                    }
                }
                Tab::Year => {
                    if let Some(ed) = &mut m.holiday_edit {
                        if ed.on_name { ed.name.insert_str(s.trim()) } else { ed.date.insert_str(s.trim()) }
                    }
                }
            }
        }
        Backspace | Delete => {
            let back = matches!(action, Backspace);
            let Some(m) = model(app) else { return };
            match m.tab {
                Tab::Global => {
                    if let Some(t) = m.global_text_mut() {
                        if back { t.backspace() } else { t.delete() }
                        m.dirty = true;
                    }
                }
                Tab::Year => {
                    if let Some(ed) = &mut m.holiday_edit {
                        let t = if ed.on_name { &mut ed.name } else { &mut ed.date };
                        if back { t.backspace() } else { t.delete() }
                    } else if m.y_focus == YearField::Hours
                        && let Some(cfg) = &cfg
                    {
                        let t = &mut m.year_draft(cfg).hours;
                        if back { t.backspace() } else { t.delete() }
                        m.dirty = true;
                    } else if matches!(m.y_focus, YearField::Holiday(_)) {
                        update(app, RemoveHoliday);
                    }
                }
            }
        }
    }
}
