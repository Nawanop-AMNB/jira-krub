use super::model::{Action, GlobalField, Model, Tab, YearField};
use crate::tui::action::Action as Global;
use crate::tui::app::App;
use crate::tui::hit::HitRegistry;
use crate::tui::theme;
use crate::tui::view::{Hint, button, centered};
use crate::tui::widgets::TextInput;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

const LABEL_W: u16 = 32;
const DAY_NAMES: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

pub fn view(frame: &mut Frame, app: &App, m: &Model, body: Rect, hits: &mut HitRegistry) -> Vec<Hint> {
    let area = centered(76.min(body.width), 22.min(body.height), body);
    let block = Block::bordered().title(" settings ").border_style(theme::accent());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    hits.click(area, Global::Nop);

    // ---- tab bar ----
    let y = inner.y;
    let mut x = inner.x + 2;
    for (tab, label) in [(Tab::Global, "Global".to_string()), (Tab::Year, format!("Year {}", m.year))] {
        let active = m.tab == tab;
        let text = if active { format!("[ {label} ]") } else { format!("  {label}  ") };
        let w = text.chars().count() as u16;
        let style = if active { theme::title() } else { theme::dim() };
        frame.render_widget(Paragraph::new(Span::styled(text, style)), Rect { x, y, width: w, height: 1 });
        hits.click(Rect { x, y, width: w, height: 1 }, Global::Settings(Action::SetTab(tab)));
        x += w + 1;
    }
    if m.tab == Tab::Year {
        frame.render_widget(Paragraph::new(Span::styled("◀ ▶", theme::accent())), Rect { x, y, width: 3, height: 1 });
        hits.click(Rect { x, y, width: 1, height: 1 }, Global::Settings(Action::PrevYear));
        hits.click(Rect { x: x + 2, y, width: 1, height: 1 }, Global::Settings(Action::NextYear));
    }
    frame.render_widget(Paragraph::new(Span::styled("Tab switches", theme::dim())), Rect { x: inner.x + inner.width.saturating_sub(13), y, width: 12, height: 1 });

    let mut y = y + 2;
    match m.tab {
        Tab::Global => y = draw_global(frame, m, inner, y, hits),
        Tab::Year => y = draw_year(frame, m, inner, y, hits),
    }

    // ---- error + buttons ----
    let by = inner.y + inner.height - 2;
    if let Some(e) = &m.error {
        let ey = by.saturating_sub(1).max(y);
        frame.render_widget(Paragraph::new(Span::styled(format!("  ✗ {e}"), theme::bad())), Rect { x: inner.x, y: ey, width: inner.width, height: 1 });
    }
    let (save_f, cancel_f) = match m.tab {
        Tab::Global => (m.g_focus == GlobalField::Save, m.g_focus == GlobalField::Cancel),
        Tab::Year => (m.y_focus == YearField::Save, m.y_focus == YearField::Cancel),
    };
    let bx = inner.x + inner.width.saturating_sub(24);
    let used = button(frame, bx, by, "Save", save_f, Global::Settings(Action::Save), hits);
    button(frame, bx + used + 2, by, "Cancel", cancel_f, Global::Settings(Action::Cancel), hits);
    let _ = app;

    vec![
        Hint::new("Tab", "switch tab", Global::Settings(Action::SwitchTab)),
        Hint::new("↑↓", "field", Global::Settings(Action::Down)),
        Hint::new("Enter", "toggle / edit", Global::Settings(Action::Activate)),
        Hint::new("^S", "save", Global::Settings(Action::Save)),
        Hint::new("Esc", "back", Global::Settings(Action::Cancel)),
    ]
}

fn label(frame: &mut Frame, inner: Rect, y: u16, text: &str, focused: bool) {
    let style = if focused { theme::accent().add_modifier(Modifier::BOLD) } else { Style::new() };
    frame.render_widget(Paragraph::new(Span::styled(format!("  {text}"), style)), Rect { x: inner.x, y, width: LABEL_W, height: 1 });
}

#[allow(clippy::too_many_arguments)]
fn text_row(frame: &mut Frame, hits: &mut HitRegistry, inner: Rect, y: u16, name: &str, input: &TextInput, width: u16, focused: bool, hint: &str, action: Global) {
    label(frame, inner, y, name, focused);
    let rect = Rect { x: inner.x + LABEL_W, y, width, height: 1 };
    frame.render_widget(Paragraph::new("["), Rect { x: rect.x - 1, y, width: 1, height: 1 });
    frame.render_widget(Paragraph::new("]"), Rect { x: rect.x + width, y, width: 1, height: 1 });
    input.render(frame, rect, Style::new(), false, focused, "");
    hits.click(Rect { x: rect.x - 1, y, width: width + 2, height: 1 }, action);
    let hx = rect.x + width + 3;
    if hx < inner.x + inner.width {
        frame.render_widget(Paragraph::new(Span::styled(hint, theme::dim())), Rect { x: hx, y, width: (inner.x + inner.width).saturating_sub(hx), height: 1 });
    }
}

fn draw_global(frame: &mut Frame, m: &Model, inner: Rect, mut y: u16, hits: &mut HitRegistry) -> u16 {
    let f = m.g_focus;
    text_row(frame, hits, inner, y, "Target hours / day", &m.hours, 4, f == GlobalField::Hours, "1–24", Global::Settings(Action::FocusGlobal(GlobalField::Hours)));
    y += 1;

    label(frame, inner, y, "Workdays", f == GlobalField::Workdays);
    let mut x = inner.x + LABEL_W;
    for (i, name) in DAY_NAMES.iter().enumerate() {
        let on = m.workdays[i];
        let cursor = f == GlobalField::Workdays && m.workday_cursor == i;
        let text = format!("[{}]{name}", if on { "x" } else { " " });
        let mut style = if on { Style::new() } else { theme::dim() };
        if cursor {
            style = style.add_modifier(Modifier::REVERSED);
        }
        let w = text.chars().count() as u16;
        frame.render_widget(Paragraph::new(Span::styled(text, style)), Rect { x, y, width: w, height: 1 });
        hits.click(Rect { x, y, width: w, height: 1 }, Global::Settings(Action::FocusGlobal(GlobalField::Workdays)));
        x += w + 1;
    }
    if f == GlobalField::Workdays {
        frame.render_widget(Paragraph::new(Span::styled("←→ pick · Enter toggle", theme::dim())), Rect { x: x + 1, y, width: (inner.x + inner.width).saturating_sub(x + 1), height: 1 });
    }
    y += 1;

    text_row(frame, hits, inner, y, "Default start time", &m.start, 6, f == GlobalField::Start, "HH:MM", Global::Settings(Action::FocusGlobal(GlobalField::Start)));
    y += 1;
    text_row(frame, hits, inner, y, "Quick-stage duration", &m.quick_stage, 7, f == GlobalField::QuickStage, "e.g. 1h, 30m", Global::Settings(Action::FocusGlobal(GlobalField::QuickStage)));
    y += 1;
    text_row(frame, hits, inner, y, "History lookback (weeks)", &m.lookback, 4, f == GlobalField::Lookback, "1–52 · changes re-sync", Global::Settings(Action::FocusGlobal(GlobalField::Lookback)));
    y += 1;

    label(frame, inner, y, "Auto-watch from search", f == GlobalField::AutoWatch);
    let text = format!("[{}]", if m.auto_watch { "x" } else { " " });
    let mut style = Style::new();
    if f == GlobalField::AutoWatch {
        style = style.add_modifier(Modifier::REVERSED);
    }
    let rect = Rect { x: inner.x + LABEL_W, y, width: 3, height: 1 };
    frame.render_widget(Paragraph::new(Span::styled(text, style)), rect);
    hits.click(rect, Global::Settings(Action::FocusGlobal(GlobalField::AutoWatch)));
    y += 2;

    let conn = "Jira connection…   ".to_string();
    let style = if f == GlobalField::Connection { theme::accent().add_modifier(Modifier::BOLD | Modifier::REVERSED) } else { theme::accent() };
    let rect = Rect { x: inner.x + 2, y, width: conn.chars().count() as u16, height: 1 };
    frame.render_widget(Paragraph::new(Span::styled(conn, style)), rect);
    hits.click(rect, Global::Settings(Action::FocusGlobal(GlobalField::Connection)));
    y += 1;
    y
}

fn draw_year(frame: &mut Frame, m: &Model, inner: Rect, mut y: u16, hits: &mut HitRegistry) -> u16 {
    let f = m.y_focus;
    let Some(d) = m.current_year() else { return y };
    text_row(frame, hits, inner, y, "Target hours / day (override)", &d.hours, 4, f == YearField::Hours && m.holiday_edit.is_none(), "blank = global", Global::Settings(Action::FocusYear(YearField::Hours)));
    y += 2;

    label(frame, inner, y, "Public holidays", matches!(f, YearField::Holiday(_)));
    if d.holidays.is_empty() && m.holiday_edit.is_none() {
        frame.render_widget(Paragraph::new(Span::styled("none", theme::dim())), Rect { x: inner.x + LABEL_W, y, width: 10, height: 1 });
        y += 1;
    }
    let rows = d.holidays.len().max(m.holiday_edit.as_ref().map(|e| e.index + 1).unwrap_or(0));
    for i in 0..rows {
        if y >= inner.y + inner.height - 3 {
            break;
        }
        let focused = f == YearField::Holiday(i);
        let x = inner.x + LABEL_W;
        match m.holiday_edit.as_ref().filter(|e| e.index == i) {
            Some(ed) => {
                let date_rect = Rect { x, y, width: 11, height: 1 };
                let name_rect = Rect { x: x + 13, y, width: (inner.x + inner.width).saturating_sub(x + 14), height: 1 };
                let err = ed.error.is_some();
                ed.date.render(frame, date_rect, if err { theme::editing_error() } else { theme::editing() }, false, !ed.on_name, "YYYY-MM-DD");
                ed.name.render(frame, name_rect, if ed.on_name { theme::editing() } else { Style::new() }, false, ed.on_name, "name");
                if let Some(e) = &ed.error {
                    frame.render_widget(Paragraph::new(Span::styled(format!("✗ {e}"), theme::bad())), Rect { x, y: y + 1, width: inner.width - LABEL_W, height: 1 });
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
                let rect = Rect { x, y, width: inner.width - LABEL_W, height: 1 };
                frame.render_widget(Paragraph::new(line), rect);
                hits.click(rect, Global::Settings(Action::FocusYear(YearField::Holiday(i))));
            }
        }
        y += 1;
    }
    let add_focused = f == YearField::AddHoliday;
    let text = "+ add holiday";
    let style = if add_focused { theme::accent().add_modifier(Modifier::REVERSED) } else { theme::accent() };
    let rect = Rect { x: inner.x + LABEL_W, y, width: text.len() as u16, height: 1 };
    frame.render_widget(Paragraph::new(Span::styled(text, style)), rect);
    hits.click(rect, Global::Settings(Action::FocusYear(YearField::AddHoliday)));
    if matches!(f, YearField::Holiday(_)) && m.holiday_edit.is_none() {
        frame.render_widget(Paragraph::new(Span::styled("   Enter edit · ⌫ remove", theme::dim())), Rect { x: rect.x + rect.width, y, width: 26, height: 1 });
    }
    y += 1;
    y
}
