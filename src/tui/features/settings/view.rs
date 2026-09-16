use super::model::{Action, GlobalField, Model, Tab, YearField};
use crate::tui::action::Action as Global;
use crate::tui::app::App;
use crate::tui::hit::HitRegistry;
use crate::tui::theme;
use crate::tui::view::{Hint, button, centered};
use crate::tui::widgets::TextInput;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

// Fixed grid: label column, one value box for every field, hint after.
const PAD: u16 = 2;
const LABEL_W: u16 = 26;
const BOX_W: u16 = 10;
const BOX_W_: u16 = BOX_W - 2; // inside the brackets
const HINT_GAP: u16 = 3;
const DAY_NAMES: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

pub fn view(frame: &mut Frame, app: &App, m: &Model, body: Rect, hits: &mut HitRegistry) -> Vec<Hint> {
    // height follows content: tab bar + rows + buttons
    let rows: u16 = match m.tab {
        Tab::Global => 10,
        Tab::Year => 4 + m.current_year().map(|d| d.holidays.len() as u16).unwrap_or(0) + u16::from(m.holiday_edit.is_some()),
    };
    let height = (rows + 5).min(body.height);
    let area = centered(70.min(body.width), height, body);
    let block = Block::bordered().title(" settings ").border_style(theme::accent());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    hits.click(area, Global::Nop);

    // ---- tab bar ----
    let y = inner.y;
    let mut x = inner.x + PAD;
    for (tab, label) in [(Tab::Global, "Global".to_string()), (Tab::Year, format!("Year {}", m.year))] {
        let active = m.tab == tab;
        let text = format!(" {label} ");
        let w = text.chars().count() as u16;
        let style = if active { theme::title() } else { theme::dim() };
        frame.render_widget(Paragraph::new(Span::styled(text, style)), Rect { x, y, width: w, height: 1 });
        hits.click(Rect { x, y, width: w, height: 1 }, Global::Settings(Action::SetTab(tab)));
        x += w + 2;
    }
    if m.tab == Tab::Year {
        frame.render_widget(Paragraph::new(Span::styled("◀ ▶", theme::accent())), Rect { x, y, width: 3, height: 1 });
        hits.click(Rect { x, y, width: 1, height: 1 }, Global::Settings(Action::PrevYear));
        hits.click(Rect { x: x + 2, y, width: 1, height: 1 }, Global::Settings(Action::NextYear));
    }
    let hint = if m.tab == Tab::Year { "Tab switches · ←→ year" } else { "Tab switches" };
    let hw = hint.chars().count() as u16;
    frame.render_widget(Paragraph::new(Span::styled(hint, theme::dim())), Rect { x: inner.x + inner.width - PAD - hw, y, width: hw, height: 1 });

    let mut y = y + 2;
    y = match m.tab {
        Tab::Global => draw_global(frame, app, m, inner, y, hits),
        Tab::Year => draw_year(frame, m, inner, y, hits),
    };

    // ---- error + buttons ----
    if let Some(e) = &m.error {
        frame.render_widget(Paragraph::new(Span::styled(format!("{:>w$}✗ {e}", "", w = PAD as usize), theme::bad())), Rect { x: inner.x, y, width: inner.width, height: 1 });
    }
    y += 1;
    let (save_f, cancel_f) = match m.tab {
        Tab::Global => (m.g_focus == GlobalField::Save, m.g_focus == GlobalField::Cancel),
        Tab::Year => (m.y_focus == YearField::Save, m.y_focus == YearField::Cancel),
    };
    let by = y.min(inner.y + inner.height - 1);
    let bx = inner.x + inner.width.saturating_sub(24);
    let used = button(frame, bx, by, "Save", save_f, Global::Settings(Action::Save), hits);
    button(frame, bx + used + 2, by, "Cancel", cancel_f, Global::Settings(Action::Cancel), hits);

    if m.editing || m.holiday_edit.is_some() {
        vec![
            Hint::new("Enter", "commit", Global::Settings(Action::Activate)),
            Hint::new("Esc", "revert", Global::Settings(Action::Revert)),
            Hint::new("^S", "save", Global::Settings(Action::Save)),
        ]
    } else {
        vec![
            Hint::new("Tab", "switch tab", Global::Settings(Action::SwitchTab)),
            Hint::new("↑↓", "field", Global::Settings(Action::Down)),
            Hint::new("Enter", "edit / toggle", Global::Settings(Action::Activate)),
            Hint::new("^S", "save", Global::Settings(Action::Save)),
            Hint::new("Esc", "back", Global::Settings(Action::Cancel)),
        ]
    }
}

fn label(frame: &mut Frame, inner: Rect, y: u16, text: &str, focused: bool) {
    let style = if focused { theme::accent().add_modifier(Modifier::BOLD) } else { Style::new() };
    frame.render_widget(Paragraph::new(Span::styled(text, style)), Rect { x: inner.x + PAD, y, width: LABEL_W, height: 1 });
}

fn value_x(inner: Rect) -> u16 {
    inner.x + PAD + LABEL_W
}

/// `[ value   ]` box at the value column; returns the x after it.
fn value_box(frame: &mut Frame, inner: Rect, y: u16, focused: bool) -> Rect {
    let x = value_x(inner);
    let style = if focused { theme::accent() } else { theme::dim() };
    frame.render_widget(Paragraph::new(Span::styled("[", style)), Rect { x, y, width: 1, height: 1 });
    frame.render_widget(Paragraph::new(Span::styled("]", style)), Rect { x: x + BOX_W - 1, y, width: 1, height: 1 });
    Rect { x: x + 1, y, width: BOX_W_, height: 1 }
}

fn hint_at(frame: &mut Frame, inner: Rect, y: u16, text: &str) {
    let x = value_x(inner) + BOX_W + HINT_GAP;
    let w = (inner.x + inner.width).saturating_sub(x + PAD);
    if w > 0 {
        frame.render_widget(Paragraph::new(Span::styled(text, theme::dim())), Rect { x, y, width: w, height: 1 });
    }
}

#[allow(clippy::too_many_arguments)]
fn text_row(frame: &mut Frame, hits: &mut HitRegistry, inner: Rect, y: u16, name: &str, input: &TextInput, (focused, editing): (bool, bool), hint: &str, action: Global) {
    label(frame, inner, y, name, focused);
    let rect = value_box(frame, inner, y, focused);
    let inner_rect = Rect { x: rect.x + 1, y, width: rect.width - 1, height: 1 };
    let open = focused && editing;
    input.render(frame, inner_rect, if open { theme::editing() } else { Style::new() }, false, open, "");
    hits.click(Rect { x: rect.x - 1, y, width: BOX_W, height: 1 }, action);
    hint_at(frame, inner, y, hint);
}

#[allow(clippy::too_many_arguments)]
fn toggle_row(frame: &mut Frame, hits: &mut HitRegistry, inner: Rect, y: u16, name: &str, on: bool, focused: bool, hint: &str, action: Global) {
    label(frame, inner, y, name, focused);
    let rect = value_box(frame, inner, y, focused);
    let (text, style) = if on { (" on", theme::good()) } else { (" off", theme::dim()) };
    let style = if focused { style.add_modifier(Modifier::BOLD) } else { style };
    frame.render_widget(Paragraph::new(Span::styled(text, style)), rect);
    hits.click(Rect { x: rect.x - 1, y, width: BOX_W, height: 1 }, action);
    hint_at(frame, inner, y, hint);
}

fn draw_global(frame: &mut Frame, app: &App, m: &Model, inner: Rect, mut y: u16, hits: &mut HitRegistry) -> u16 {
    let f = m.g_focus;
    let ed = m.editing;
    text_row(frame, hits, inner, y, "Target hours / day", &m.hours, (f == GlobalField::Hours, ed), "1–24", Global::Settings(Action::FocusGlobal(GlobalField::Hours)));
    y += 1;

    // workdays: seven chips, on = bright, off = dim, cursor = reversed
    let wf = f == GlobalField::Workdays;
    label(frame, inner, y, "Workdays", wf);
    let mut x = value_x(inner) + 1;
    for (i, name) in DAY_NAMES.iter().enumerate() {
        let on = m.workdays[i];
        let mut style = if on { Style::new().fg(Color::Green).add_modifier(Modifier::BOLD) } else { theme::dim() };
        if wf && ed && m.workday_cursor == i {
            style = style.add_modifier(Modifier::REVERSED);
        }
        let text = format!(" {name} ");
        let w = text.len() as u16;
        frame.render_widget(Paragraph::new(Span::styled(text, style)), Rect { x, y, width: w, height: 1 });
        hits.click(Rect { x, y, width: w, height: 1 }, Global::Settings(Action::FocusGlobal(GlobalField::Workdays)));
        x += w;
        if i == 4 {
            x += 1;
        }
    }
    if wf {
        let hint = if ed { "←→ pick · Space toggles · Enter done" } else { "Enter to edit" };
        let hx = x + 2;
        frame.render_widget(Paragraph::new(Span::styled(hint, theme::dim())), Rect { x: hx, y, width: (inner.x + inner.width).saturating_sub(hx + PAD), height: 1 });
    }
    y += 1;

    text_row(frame, hits, inner, y, "Default start time", &m.start, (f == GlobalField::Start, ed), "HH:MM", Global::Settings(Action::FocusGlobal(GlobalField::Start)));
    y += 1;
    text_row(frame, hits, inner, y, "Quick-stage duration", &m.quick_stage, (f == GlobalField::QuickStage, ed), "e.g. 1h, 30m", Global::Settings(Action::FocusGlobal(GlobalField::QuickStage)));
    y += 1;
    text_row(frame, hits, inner, y, "History lookback", &m.lookback, (f == GlobalField::Lookback, ed), "weeks · change re-syncs", Global::Settings(Action::FocusGlobal(GlobalField::Lookback)));
    y += 1;
    toggle_row(frame, hits, inner, y, "Auto-watch from search", m.auto_watch, f == GlobalField::AutoWatch, "Enter toggles", Global::Settings(Action::FocusGlobal(GlobalField::AutoWatch)));
    y += 2;

    // connection link + current target, dim
    let cf = f == GlobalField::Connection;
    let text = "Jira connection…";
    let style = if cf { theme::accent().add_modifier(Modifier::BOLD | Modifier::REVERSED) } else { theme::accent() };
    let rect = Rect { x: inner.x + PAD, y, width: text.chars().count() as u16, height: 1 };
    frame.render_widget(Paragraph::new(Span::styled(text, style)), rect);
    hits.click(rect, Global::Settings(Action::FocusGlobal(GlobalField::Connection)));
    if let Some(c) = &app.config {
        let who = format!("{} · {}", c.credentials.site, c.credentials.email);
        let x = value_x(inner) + 1;
        frame.render_widget(Paragraph::new(Span::styled(who, theme::dim())), Rect { x, y, width: (inner.x + inner.width).saturating_sub(x + PAD), height: 1 });
    }
    y += 2;
    y
}

fn draw_year(frame: &mut Frame, m: &Model, inner: Rect, mut y: u16, hits: &mut HitRegistry) -> u16 {
    let f = m.y_focus;
    let Some(d) = m.current_year() else { return y };
    text_row(frame, hits, inner, y, "Target hours / day", &d.hours, (f == YearField::Hours && m.holiday_edit.is_none(), m.editing), "blank = global", Global::Settings(Action::FocusYear(YearField::Hours)));
    y += 1;

    label(frame, inner, y, "Public holidays", matches!(f, YearField::Holiday(_)));
    let x = value_x(inner) + 1;
    let row_w = (inner.x + inner.width).saturating_sub(x + PAD);
    if d.holidays.is_empty() && m.holiday_edit.is_none() {
        frame.render_widget(Paragraph::new(Span::styled("none", theme::dim())), Rect { x, y, width: 4, height: 1 });
        y += 1;
    }
    let rows = d.holidays.len().max(m.holiday_edit.as_ref().map(|e| e.index + 1).unwrap_or(0));
    for i in 0..rows {
        if y >= inner.y + inner.height - 3 {
            break;
        }
        let focused = f == YearField::Holiday(i);
        match m.holiday_edit.as_ref().filter(|e| e.index == i) {
            Some(ed) => {
                let date_rect = Rect { x, y, width: 11, height: 1 };
                let name_rect = Rect { x: x + 12, y, width: row_w.saturating_sub(12), height: 1 };
                let err = ed.error.is_some();
                ed.date.render(frame, date_rect, if err { theme::editing_error() } else if ed.on_name { Style::new() } else { theme::editing() }, false, !ed.on_name, "MM-DD");
                ed.name.render(frame, name_rect, if ed.on_name { theme::editing() } else { Style::new() }, false, ed.on_name, "name");
                if let Some(e) = &ed.error {
                    frame.render_widget(Paragraph::new(Span::styled(format!("✗ {e}"), theme::bad())), Rect { x, y: y + 1, width: row_w, height: 1 });
                    y += 1;
                }
            }
            None => {
                let row = &d.holidays[i];
                let mut style = Style::new();
                if focused {
                    style = style.add_modifier(Modifier::REVERSED);
                }
                let line = Line::from(vec![Span::styled(format!("{:<12}", row.date), style), Span::styled(row.name.clone(), style)]);
                let rect = Rect { x, y, width: row_w, height: 1 };
                frame.render_widget(Paragraph::new(line), rect);
                hits.click(rect, Global::Settings(Action::FocusYear(YearField::Holiday(i))));
            }
        }
        y += 1;
    }
    let add_focused = f == YearField::AddHoliday;
    let text = "+ add holiday";
    let style = if add_focused { theme::accent().add_modifier(Modifier::REVERSED) } else { theme::accent() };
    let rect = Rect { x, y, width: text.len() as u16, height: 1 };
    frame.render_widget(Paragraph::new(Span::styled(text, style)), rect);
    hits.click(rect, Global::Settings(Action::FocusYear(YearField::AddHoliday)));
    if matches!(f, YearField::Holiday(_)) && m.holiday_edit.is_none() {
        frame.render_widget(Paragraph::new(Span::styled("   Enter edit · ⌫ remove", theme::dim())), Rect { x: rect.x + rect.width, y, width: 26.min(row_w.saturating_sub(rect.width)), height: 1 });
    }
    y += 2;
    y
}
