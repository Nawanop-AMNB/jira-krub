use super::model::{Action, Model};
use crate::domain::{DayStatus, DaySummary, duration};
use crate::tui::action::{Action as Global, PushScope};
use crate::tui::app::App;
use crate::tui::hit::{HitArea, HitRegistry};
use crate::tui::theme;
use crate::tui::view::Hint;
use chrono::Datelike;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

const BAR_WIDTH: usize = 24;

pub fn view(frame: &mut Frame, app: &App, m: &Model, body: Rect, hits: &mut HitRegistry) -> Vec<Hint> {
    let [header, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(body);
    let target = app.target_seconds();
    let summaries: Vec<DaySummary> = m
        .week
        .days()
        .iter()
        .map(|d| DaySummary::compute(*d, app.today, target, app.ledger.entries(), &app.remote.worklogs))
        .collect();

    draw_header(frame, app, m, &summaries, header, hits);
    draw_days(frame, app, m, &summaries, list, hits);

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

fn draw_header(frame: &mut Frame, app: &App, m: &Model, summaries: &[DaySummary], area: Rect, hits: &mut HitRegistry) {
    let week_total: u64 = summaries.iter().map(|s| s.total()).sum();
    let staged: usize = summaries.iter().map(|s| s.staged_count).sum();
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
    push(&mut spans, "  ".into(), Style::new(), None);
    push(&mut spans, format!("◀ {prev}"), theme::accent(), Some(Global::Week(Action::PrevWeek)));
    push(&mut spans, "  ".into(), Style::new(), None);
    push(&mut spans, label, theme::bold(), Some(Global::Week(Action::Today)));
    push(&mut spans, "  ".into(), Style::new(), None);
    push(&mut spans, format!("{next} ▶"), theme::accent(), Some(Global::Week(Action::NextWeek)));
    push(&mut spans, "     ".into(), Style::new(), None);
    let weekly = app.weekly_target_seconds();
    let total_style = if week_total >= weekly { theme::good() } else { theme::warn() };
    push(&mut spans, format!("{} / {}", hours(week_total), hours(weekly)), total_style, None);
    push(&mut spans, "   ".into(), Style::new(), None);
    if staged > 0 {
        push(&mut spans, format!("{staged} staged"), theme::warn(), Some(Global::PushRequest(PushScope::Week(m.week))));
    }
    if app.remote.syncing {
        push(&mut spans, "  ⟳".into(), theme::dim(), None);
    } else if app.remote.offline {
        push(&mut spans, "  offline".into(), theme::bad(), None);
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_days(frame: &mut Frame, app: &App, m: &Model, summaries: &[DaySummary], area: Rect, hits: &mut HitRegistry) {
    let block = Block::bordered();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    hits.add(HitArea {
        rect: area,
        scroll_up: Some(Global::Week(Action::PrevWeek)),
        scroll_down: Some(Global::Week(Action::NextWeek)),
        ..Default::default()
    });
    let target = app.target_seconds();
    for (i, s) in summaries.iter().enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.y + inner.height {
            break;
        }
        let row = Rect { x: inner.x, y, width: inner.width, height: 1 };
        let selected = i == m.selected;
        let is_today = s.date == app.today;

        let day_label = format!("{} {:02}", s.date.weekday(), s.date.day());
        let (bar, hours_txt, status_txt, status_style) = match s.status {
            DayStatus::Weekend => (String::new(), String::new(), String::new(), theme::dim()),
            DayStatus::Future => ("─".repeat(BAR_WIDTH), "–".into(), "future".into(), theme::dim()),
            DayStatus::Full => (bar(s, target), hours(s.total()), "✓".into(), theme::good()),
            DayStatus::Short => (bar(s, target), hours(s.total()), format!("need {}", hours(s.remaining(target))), theme::warn()),
            DayStatus::Empty => (bar(s, target), "0".into(), "empty".into(), theme::bad()),
            DayStatus::TodayEmpty => (bar(s, target), "0".into(), format!("need {}", hours(target)), theme::warn()),
        };
        let staged_txt = if s.staged_count > 0 { format!("{} staged", s.staged_count) } else { String::new() };

        let mut day_style = if is_today { theme::accent().add_modifier(ratatui::style::Modifier::BOLD) } else { Style::new() };
        // Selection: reverse the text cells only. Reversing the bar would swap
        // its colour into the background and hide the green/yellow fill.
        let hi = |st: Style| if selected { st.add_modifier(ratatui::style::Modifier::REVERSED) } else { st };
        day_style = hi(day_style);
        let line = Line::from(vec![
            Span::styled(if selected { "▶ " } else { "  " }, theme::accent()),
            Span::styled(format!(" {day_label:<7}"), day_style),
            Span::raw(" "),
            Span::styled(format!("{bar:<width$}", width = BAR_WIDTH), status_style),
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
}

fn bar(s: &DaySummary, target: u64) -> String {
    if target == 0 {
        return String::new();
    }
    let cell = |secs: u64| ((secs as f64 / target as f64) * BAR_WIDTH as f64).round() as usize;
    let pushed = cell(s.pushed_seconds).min(BAR_WIDTH);
    let staged = cell(s.staged_seconds).min(BAR_WIDTH - pushed);
    let rest = BAR_WIDTH - pushed - staged;
    format!("{}{}{}", "█".repeat(pushed), "▓".repeat(staged), "░".repeat(rest))
}

/// `8h`, `6h15`, `0`
pub fn hours(secs: u64) -> String {
    if secs == 0 {
        return "0".into();
    }
    duration::format(secs).replace(' ', "")
}
