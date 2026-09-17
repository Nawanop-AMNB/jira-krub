use super::model::{Action, Model, open_count, visible_groups};
use crate::domain::{Week, due_label, updated_label};
use crate::tui::action::Action as Global;
use crate::tui::app::App;
use crate::tui::hit::{HitArea, HitRegistry};
use crate::tui::theme;
use crate::tui::view::Hint;
use crate::tui::widgets::text::{display_width, pad_to_width};
use crate::tui::widgets::truncate_to_width;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

// Fixed columns, so `due` and `updated` start at the same x on every row
// whatever the key, the summary or a missing due date (R9c).
const C_MARK: u16 = 2;
const C_KEY: u16 = 10;
/// `due 05 Jan 2027` is the longest label the domain produces.
const C_DUE: u16 = 16;
/// `updated 12d ago`; the column runs to the end of the row.
const C_UPD: u16 = 16;
const GAP: u16 = 1;
const C_SUM_MIN: u16 = 8;

fn summary_width(inner_width: u16) -> u16 {
    inner_width.saturating_sub(C_MARK + C_KEY + GAP + GAP + C_DUE + C_UPD).max(C_SUM_MIN)
}

pub fn view(frame: &mut Frame, app: &App, m: &Model, body: Rect, hits: &mut HitRegistry) -> Vec<Hint> {
    let total = open_count(app);
    let mut title = if app.remote.syncing { format!(" my tasks · {total} · syncing… ") } else { format!(" my tasks · {total} ") };
    if app.remote.offline {
        title.push_str("· offline ");
    }
    let block = Block::bordered().title(title).border_style(theme::accent());
    let inner = block.inner(body);
    frame.render_widget(block, body);
    hits.add(HitArea { rect: body, scroll_up: Some(Global::Tasks(Action::Up)), scroll_down: Some(Global::Tasks(Action::Down)), ..Default::default() });

    if total == 0 && !app.remote.syncing {
        frame.render_widget(
            Paragraph::new(Span::styled("   nothing assigned to you · r sync", theme::dim())),
            Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 },
        );
        return hints();
    }

    draw_filter(frame, m, inner, hits);
    draw_list(frame, app, m, Rect { y: inner.y + 1, height: inner.height.saturating_sub(1), ..inner }, hits);
    hints()
}

fn hints() -> Vec<Hint> {
    vec![
        Hint::new("Enter", "open in Jira", Global::Tasks(Action::Open)),
        Hint::new("↑↓", "move", Global::Nop),
        Hint::new("/", "filter", Global::Tasks(Action::FocusFilter)),
        Hint::new("r", "sync", Global::Refresh),
        Hint::new("Tab", "worklogs", Global::SwitchMainTab),
        Hint::new(",", "settings", Global::OpenSettings),
        Hint::new("q", "quit", Global::Quit),
    ]
}

fn draw_filter(frame: &mut Frame, m: &Model, inner: Rect, hits: &mut HitRegistry) {
    let y = inner.y;
    let icon = if m.filter_focused { theme::accent() } else { theme::dim() };
    frame.render_widget(Paragraph::new(Span::styled(" 🔍 ", icon)), Rect { x: inner.x, y, width: 4, height: 1 });
    let rect = Rect { x: inner.x + 4, y, width: inner.width.saturating_sub(5), height: 1 };
    m.filter.render(frame, rect, Style::new(), false, m.filter_focused, "type to filter");
    hits.click(rect, Global::Tasks(Action::FocusFilter));
}

/// One rendered line: a group header (no row) or an issue row.
struct L {
    text: Line<'static>,
    row: Option<usize>,
}

fn draw_list(frame: &mut Frame, app: &App, m: &Model, area: Rect, hits: &mut HitRegistry) {
    let groups = visible_groups(app, m);
    if groups.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("   no match", theme::dim())),
            Rect { x: area.x, y: area.y, width: area.width, height: 1 },
        );
        return;
    }

    let week = Week::containing(app.today);
    let sum_w = summary_width(area.width);
    let mut lines: Vec<L> = Vec::new();
    let mut i = 0usize;
    for g in &groups {
        let label = format!(" ─ {} · {} ", g.status, g.issues.len());
        let pad = (area.width as usize).saturating_sub(display_width(&label));
        lines.push(L { text: Line::from(Span::styled(format!("{label}{}", "─".repeat(pad)), theme::dim())), row: None });
        for issue in &g.issues {
            let selected = !m.filter_focused && i == m.selected;
            let due = due_label(issue.due, app.today, &week).unwrap_or_default();
            let due_style = if due.starts_with("overdue") || due == "due today" { theme::warn() } else { theme::dim() };
            lines.push(L {
                text: Line::from(vec![
                    Span::styled(if selected { "▶ " } else { "  " }, theme::accent()),
                    Span::styled(pad_to_width(&truncate_to_width(issue.key.as_str(), C_KEY as usize), C_KEY as usize), theme::issue_key()),
                    Span::raw(" ".repeat(GAP as usize)),
                    Span::raw(pad_to_width(&truncate_to_width(&issue.summary, sum_w as usize), sum_w as usize)),
                    Span::raw(" ".repeat(GAP as usize)),
                    Span::styled(pad_to_width(&due, C_DUE as usize), due_style),
                    Span::styled(updated_label(issue.updated, app.today), theme::dim()),
                ]),
                row: Some(i),
            });
            i += 1;
        }
    }

    // scroll so the selected row stays visible
    let h = area.height as usize;
    let sel_line = lines.iter().position(|l| l.row == Some(m.selected)).unwrap_or(0);
    let offset = if h == 0 || sel_line < h { 0 } else { sel_line + 1 - h };
    for (k, l) in lines.iter().skip(offset).take(h).enumerate() {
        let rect = Rect { x: area.x, y: area.y + k as u16, width: area.width, height: 1 };
        frame.render_widget(Paragraph::new(l.text.clone()), rect);
        if let Some(row) = l.row {
            hits.add(HitArea {
                rect,
                click: Some(Global::Tasks(Action::Select(row))),
                double: Some(Global::Tasks(Action::SelectAndOpen(row))),
                scroll_up: Some(Global::Tasks(Action::Up)),
                scroll_down: Some(Global::Tasks(Action::Down)),
                ..Default::default()
            });
        }
    }
}
