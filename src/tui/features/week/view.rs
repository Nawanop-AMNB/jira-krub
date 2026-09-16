use super::model::{Action, Model};
use crate::domain::{DayStatus, DaySummary, duration};
use crate::tui::action::{Action as Global, PushScope};
use crate::tui::app::App;
use crate::tui::features::day::rows::{PrepareRow, prepare_rows_for};
use crate::tui::hit::{HitArea, HitRegistry};
use crate::tui::theme;
use crate::tui::view::Hint;
use chrono::Datelike;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

// fixed columns around the bar: "▶ " + " Wed 16 " + gap | gap + hours + status + staged
const LEFT_W: u16 = 2 + 8 + 1;
const RIGHT_W: u16 = 2 + 8 + 12 + 10;
const MIN_BAR: u16 = 10;

pub fn view(frame: &mut Frame, app: &App, m: &Model, body: Rect, hits: &mut HitRegistry) -> Vec<Hint> {
    // 7 days + divider + week total, inside a border
    let box_h = (7 + 2 + 2).min(body.height.saturating_sub(1));
    let [header, week_box, below] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(box_h), Constraint::Min(0)]).areas(body);
    let calendar = app.calendar();
    let summaries: Vec<DaySummary> = m
        .week
        .days()
        .iter()
        .map(|d| DaySummary::compute(*d, app.today, &calendar, app.ledger.entries(), &app.remote.worklogs))
        .collect();

    draw_header(frame, app, m, header, hits);
    draw_week(frame, app, m, &summaries, &calendar, week_box, hits);
    if below.height >= 3 {
        draw_day_preview(frame, app, m, below);
    }

    vec![
        Hint::new("Enter", "open day", Global::Week(Action::OpenDay(m.selected))),
        Hint::new("a", "add", Global::Week(Action::AddEntry)),
        Hint::new("p", "push", Global::Week(Action::PushWeek)),
        Hint::new("←→", "week", Global::Nop),
        Hint::new("t", "today", Global::Week(Action::Today)),
        Hint::new("r", "sync", Global::Refresh),
        Hint::new(",", "settings", Global::OpenSettings),
        Hint::new("q", "quit", Global::Quit),
    ]
}

fn draw_header(frame: &mut Frame, app: &App, m: &Model, area: Rect, hits: &mut HitRegistry) {
    let prev = m.week.prev().monday().format("%d %b").to_string();
    let next = m.week.next().monday().format("%d %b").to_string();
    let label = format!("[ {} → {} ]", m.week.monday().format("%d %b"), m.week.sunday().format("%d %b %Y"));

    let mut spans: Vec<Span> = Vec::new();
    let mut x = area.x;
    let mut push = |spans: &mut Vec<Span>, text: String, style: Style, action: Option<Global>| {
        let w = text.chars().count() as u16;
        if let Some(a) = action {
            hits.click(Rect { x, y: area.y, width: w, height: 1 }, a);
        }
        spans.push(Span::styled(text, style));
        x += w;
    };
    push(&mut spans, " jira-krub ".into(), theme::title(), None);
    push(&mut spans, "   ".into(), Style::new(), None);
    push(&mut spans, format!("◀ {prev}"), theme::accent(), Some(Global::Week(Action::PrevWeek)));
    push(&mut spans, "  ".into(), Style::new(), None);
    push(&mut spans, label, theme::bold(), Some(Global::Week(Action::Today)));
    push(&mut spans, "  ".into(), Style::new(), None);
    push(&mut spans, format!("{next} ▶"), theme::accent(), Some(Global::Week(Action::NextWeek)));
    let staged_n = app.ledger.staged_count();
    if staged_n > 0 {
        push(&mut spans, format!("   ⚠ {staged_n} staged not pushed — p"), theme::warn(), Some(Global::PushRequest(PushScope::Week(m.week))));
    }
    if app.remote.syncing {
        push(&mut spans, "   ⟳ syncing".into(), theme::dim(), None);
    } else if app.remote.offline {
        push(&mut spans, "   offline".into(), theme::bad(), None);
    } else if !app.remote.display_name.is_empty() {
        push(&mut spans, format!("   {}", app.remote.display_name), theme::dim(), None);
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_week(frame: &mut Frame, app: &App, m: &Model, summaries: &[DaySummary], calendar: &crate::domain::WorkCalendar, area: Rect, hits: &mut HitRegistry) {
    let block = Block::bordered().title(" week ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    hits.add(HitArea {
        rect: area,
        scroll_up: Some(Global::Week(Action::PrevWeek)),
        scroll_down: Some(Global::Week(Action::NextWeek)),
        ..Default::default()
    });
    let bar_w = inner.width.saturating_sub(LEFT_W + RIGHT_W).max(MIN_BAR) as usize;

    for (i, s) in summaries.iter().enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.y + inner.height {
            break;
        }
        let row = Rect { x: inner.x, y, width: inner.width, height: 1 };
        let selected = i == m.selected;
        let is_today = s.date == app.today;

        let day_label = format!("{} {:02}", s.date.weekday(), s.date.day());
        let target = s.target;
        let (bar, hours_txt, status_txt, status_style) = match s.status {
            DayStatus::Off => {
                // Holidays are named; plain weekends stay blank unless hours were logged.
                let name = calendar.holiday(s.date).map(|n| format!("off · {n}"));
                let logged = s.pushed_seconds + s.staged_seconds > 0;
                match (name, logged) {
                    (Some(n), _) => (String::new(), if logged { hours(s.pushed_seconds) } else { String::new() }, n, theme::dim()),
                    (None, true) => (String::new(), hours(s.pushed_seconds), "off".into(), theme::dim()),
                    (None, false) => (String::new(), String::new(), String::new(), theme::dim()),
                }
            }
            DayStatus::Future => ("─".repeat(bar_w), "–".into(), "future".into(), theme::dim()),
            DayStatus::Full => (bar(s, target, bar_w), hours(s.pushed_seconds), "✓".into(), theme::good()),
            DayStatus::Short => (bar(s, target, bar_w), hours(s.pushed_seconds), format!("need {}", hours(s.remaining())), theme::warn()),
            DayStatus::Empty => (bar(s, target, bar_w), "0".into(), "empty".into(), theme::bad()),
            DayStatus::TodayEmpty => (bar(s, target, bar_w), "0".into(), format!("need {}", hours(target)), theme::warn()),
        };
        let staged_txt = if s.staged_count > 0 { format!("+{} staged", hours(s.staged_seconds)) } else { String::new() };

        let mut day_style = if is_today { theme::accent().add_modifier(Modifier::BOLD) } else { Style::new() };
        // Selection: reverse the text cells only. Reversing the bar would swap
        // its colour into the background and hide the green/yellow fill.
        let hi = |st: Style| if selected { st.add_modifier(Modifier::REVERSED) } else { st };
        day_style = hi(day_style);
        let line = Line::from(vec![
            Span::styled(if selected { "▶ " } else { "  " }, theme::accent()),
            Span::styled(format!(" {day_label:<7}"), day_style),
            Span::raw(" "),
            Span::styled(format!("{bar:<bar_w$}"), status_style),
            Span::raw("  "),
            Span::styled(format!("{hours_txt:<8}"), hi(theme::bold())),
            Span::styled(format!("{status_txt:<12}"), hi(status_style)),
            Span::styled(staged_txt, hi(theme::warn())),
        ]);
        frame.render_widget(Paragraph::new(line), row);
        hits.add(HitArea {
            rect: row,
            click: Some(Global::Week(Action::Select(i))),
            double: Some(Global::Week(Action::OpenDay(i))),
            ..Default::default()
        });
    }

    // divider + week total
    let y = inner.y + 7;
    if y + 1 < inner.y + inner.height {
        frame.render_widget(
            Paragraph::new(Span::styled("─".repeat(inner.width as usize), theme::dim())),
            Rect { x: inner.x, y, width: inner.width, height: 1 },
        );
        let weekly = calendar.target_between(m.week.monday(), m.week.sunday());
        let pushed: u64 = summaries.iter().map(|s| s.pushed_seconds).sum();
        let staged: u64 = summaries.iter().map(|s| s.staged_seconds).sum();
        let staged_n: usize = summaries.iter().map(|s| s.staged_count).sum();
        let style = if pushed >= weekly { theme::good() } else { theme::warn() };
        let bar_txt = fill_bar(pushed, staged, weekly, bar_w);
        let staged_txt = if staged_n > 0 { format!("+{} staged", hours(staged)) } else { String::new() };
        let row = Rect { x: inner.x, y: y + 1, width: inner.width, height: 1 };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!(" {:<7}", "week"), theme::bold()),
                Span::raw(" "),
                Span::styled(format!("{bar_txt:<bar_w$}"), style),
                Span::raw("  "),
                Span::styled(format!("{:<8}", hours(pushed)), theme::bold()),
                Span::styled(format!("{:<12}", format!("/ {}", hours(weekly))), theme::dim()),
                Span::styled(staged_txt, theme::warn()),
            ])),
            row,
        );
        if staged_n > 0 {
            hits.click(row, Global::PushRequest(PushScope::Week(m.week)));
        }
    }
}

/// Read-only list of the selected day's rows, same shape as the day view.
fn draw_day_preview(frame: &mut Frame, app: &App, m: &Model, area: Rect) {
    let date = m.selected_date();
    let rows = prepare_rows_for(app, date);
    let title = format!(" {} ", date.format("%a %d %b"));
    let block = Block::bordered().title(title).border_style(theme::dim());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if rows.rows.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("   nothing logged · a to add · Enter to open", theme::dim())),
            Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 },
        );
        return;
    }
    let mut y = inner.y;
    for (i, row) in rows.rows.iter().enumerate() {
        if y >= inner.y + inner.height {
            break;
        }
        if i == rows.pending_len && i > 0 {
            frame.render_widget(Paragraph::new(Span::styled(" ─ in jira ─", theme::dim())), Rect { x: inner.x, y, width: inner.width, height: 1 });
            y += 1;
            if y >= inner.y + inner.height {
                break;
            }
        }
        let in_jira = i >= rows.pending_len;
        let (mark, mark_style) = match row {
            PrepareRow::Local(e) if e.is_failed() => ("!", theme::bad()),
            PrepareRow::Local(e) if e.is_deleted() => ("✗", theme::bad()),
            PrepareRow::Local(e) if e.is_modified() => ("~", theme::warn()),
            PrepareRow::Local(e) if e.needs_push() => ("+", theme::warn()),
            _ => (" ", theme::dim()),
        };
        let summary = app.remote.issue(row.key()).map(|i| i.summary.clone()).unwrap_or_default();
        let desc = row.title();
        let text_style = if in_jira { theme::dim() } else { Style::new() };
        let sum_w = 24usize;
        let used = 2 + 7 + 9 + 11 + sum_w + 2;
        let desc_w = (inner.width as usize).saturating_sub(used + 3);
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(mark, mark_style),
            Span::styled(format!("{:<7}", row.start().to_string()), text_style),
            Span::styled(format!("{:<9}", duration::format(row.seconds())), text_style),
            Span::styled(format!("{:<11}", row.key().to_string()), if in_jira { theme::dim() } else { theme::issue_key() }),
            Span::styled(format!("{:<sum_w$}", truncate(&summary, sum_w)), theme::dim()),
            Span::raw("  "),
            Span::styled(truncate(if desc.trim().is_empty() { "(no description)" } else { &desc }, desc_w), text_style),
        ]);
        frame.render_widget(Paragraph::new(line), Rect { x: inner.x, y, width: inner.width, height: 1 });
        if in_jira {
            frame.render_widget(Paragraph::new(Span::styled("✓", theme::good())), Rect { x: inner.x + inner.width - 1, y, width: 1, height: 1 });
        }
        y += 1;
    }
}

fn bar(s: &DaySummary, target: u64, width: usize) -> String {
    fill_bar(s.pushed_seconds, s.staged_seconds, target, width)
}

/// `█` pushed, `▓` staged, `░` remaining, scaled to `width`.
fn fill_bar(pushed_secs: u64, staged_secs: u64, target: u64, width: usize) -> String {
    if target == 0 || width == 0 {
        return String::new();
    }
    let cell = |secs: u64| ((secs as f64 / target as f64) * width as f64).round() as usize;
    let pushed = cell(pushed_secs).min(width);
    let staged = cell(staged_secs).min(width - pushed);
    let rest = width - pushed - staged;
    format!("{}{}{}", "█".repeat(pushed), "▓".repeat(staged), "░".repeat(rest))
}

/// `8h`, `6h15`, `0`
pub fn hours(secs: u64) -> String {
    if secs == 0 {
        return "0".into();
    }
    duration::format(secs).replace(' ', "")
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}
