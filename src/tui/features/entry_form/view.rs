use super::model::{Action, Field, Model};
use crate::domain::{DaySummary, duration};
use crate::tui::action::Action as Global;
use crate::tui::app::App;
use crate::tui::hit::HitRegistry;
use crate::tui::theme;
use crate::tui::view::{Hint, button, centered};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

const LABEL_W: u16 = 12;

pub fn view(frame: &mut Frame, app: &App, m: &Model, body: Rect, hits: &mut HitRegistry) -> Vec<Hint> {
    let suggestions = if m.focus == Field::Issue { m.suggestions() } else { Vec::new() };
    let height = (14 + suggestions.len() as u16).min(body.height);
    let area = centered(64.min(body.width), height, body);
    frame.render_widget(Clear, area);
    let block = Block::bordered().title(" log work ").border_style(theme::accent());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    hits.click(area, Global::Nop); // swallow clicks on the dialog background

    let mut y = inner.y;
    let w = inner.width.saturating_sub(LABEL_W + 2);
    let fx = inner.x + LABEL_W;

    // Issue
    label(frame, inner, y, "Issue", m.focus == Field::Issue);
    m.issue.render(frame, Rect { x: fx, y, width: w.min(14), height: 1 }, Style::new(), false, m.focus == Field::Issue, "key or text");
    hits.click(Rect { x: fx, y, width: w, height: 1 }, Global::Form(Action::Focus(Field::Issue)));
    if let Some(s) = m.issue_summary() {
        let sx = fx + 15;
        frame.render_widget(
            Paragraph::new(Span::styled(truncate(s, (inner.x + inner.width).saturating_sub(sx) as usize), theme::dim())),
            Rect { x: sx, y, width: (inner.x + inner.width).saturating_sub(sx), height: 1 },
        );
    }
    y += 1;
    for (i, s) in suggestions.iter().enumerate() {
        let style = if i == m.suggestion_sel { theme::selected() } else { theme::dim() };
        let text = format!("{:<10} {}", s.key, truncate(&s.summary, w.saturating_sub(11) as usize));
        frame.render_widget(Paragraph::new(Span::styled(text, style)), Rect { x: fx, y, width: w, height: 1 });
        y += 1;
    }

    // Date
    label(frame, inner, y, "Date", m.focus == Field::Date);
    let date_txt = m.date.format("%a %d %b %Y").to_string();
    let date_style = if m.focus == Field::Date { theme::accent() } else { Style::new() };
    frame.render_widget(Paragraph::new(Span::styled(&date_txt, date_style)), Rect { x: fx, y, width: 16, height: 1 });
    hits.click(Rect { x: fx, y, width: 16, height: 1 }, Global::Form(Action::Focus(Field::Date)));
    arrows(frame, hits, fx + 18, y, ("◀", "▶"), (Global::Form(Action::DateShift(-1)), Global::Form(Action::DateShift(1))));
    y += 1;

    // Start
    label(frame, inner, y, "Start", m.focus == Field::Start);
    m.start.render(frame, Rect { x: fx, y, width: 6, height: 1 }, Style::new(), false, m.focus == Field::Start, "09:00");
    hits.click(Rect { x: fx, y, width: 6, height: 1 }, Global::Form(Action::Focus(Field::Start)));
    arrows(frame, hits, fx + 8, y, ("▲", "▼"), (Global::Form(Action::StartStep(15)), Global::Form(Action::StartStep(-15))));
    frame.render_widget(Paragraph::new(Span::styled("15m", theme::dim())), Rect { x: fx + 12, y, width: 3, height: 1 });
    y += 1;

    // Duration
    label(frame, inner, y, "Duration", m.focus == Field::Duration);
    m.duration.render(frame, Rect { x: fx, y, width: 12, height: 1 }, Style::new(), false, m.focus == Field::Duration, "1h30m");
    hits.click(Rect { x: fx, y, width: 12, height: 1 }, Global::Form(Action::Focus(Field::Duration)));
    frame.render_widget(Paragraph::new(Span::styled("e.g. 2h · 1h30m · 90m", theme::dim())), Rect { x: fx + 14, y, width: w.saturating_sub(14), height: 1 });
    y += 1;

    // Title
    label(frame, inner, y, "Descr.", m.focus == Field::Title);
    m.title.render(frame, Rect { x: fx, y, width: w, height: 1 }, Style::new(), false, m.focus == Field::Title, "what you did");
    hits.click(Rect { x: fx, y, width: w, height: 1 }, Global::Form(Action::Focus(Field::Title)));
    y += 1;

    // Detail
    label(frame, inner, y, "Detail", m.focus == Field::Detail);
    m.detail.render(frame, Rect { x: fx, y, width: w, height: 1 }, Style::new(), false, m.focus == Field::Detail, "optional");
    hits.click(Rect { x: fx, y, width: w, height: 1 }, Global::Form(Action::Focus(Field::Detail)));
    y += 2;

    // Info line
    let target = app.target_seconds();
    let s = DaySummary::compute(m.date, app.today, target, app.ledger.entries(), &app.remote.worklogs);
    let on_issue: u64 = m
        .resolve_issue()
        .map(|k| {
            app.ledger
                .entries_on(m.date)
                .iter()
                .filter(|e| e.issue_key == k)
                .map(|e| e.seconds)
                .sum::<u64>()
                + app.remote.worklogs.iter().filter(|w| w.issue_key == k && w.local_date() == m.date).map(|w| w.seconds).sum::<u64>()
        })
        .unwrap_or(0);
    let info = format!(
        "on issue today {} · pushed {} · need {} · staged {}",
        fmt0(on_issue),
        fmt0(s.pushed_seconds),
        fmt0(s.remaining(target)),
        fmt0(s.staged_seconds)
    );
    frame.render_widget(Paragraph::new(Span::styled(format!("  {info}"), theme::dim())), Rect { x: inner.x, y, width: inner.width, height: 1 });
    y += 1;
    if let Some(e) = &m.error {
        frame.render_widget(Paragraph::new(Span::styled(format!("  ✗ {e}"), theme::bad())), Rect { x: inner.x, y, width: inner.width, height: 1 });
    }
    y += 1;

    let bx = inner.x + inner.width.saturating_sub(24);
    let used = button(frame, bx, y, "Save", false, Global::Form(Action::Save), hits);
    button(frame, bx + used + 2, y, "Cancel", false, Global::Form(Action::Cancel), hits);

    vec![
        Hint::new("Tab", "next field", Global::Form(Action::FocusNext)),
        Hint::new("Enter", "save", Global::Form(Action::Save)),
        Hint::new("Esc", "cancel", Global::Form(Action::Cancel)),
    ]
}

fn label(frame: &mut Frame, inner: Rect, y: u16, text: &str, focused: bool) {
    let style = if focused { theme::accent().add_modifier(ratatui::style::Modifier::BOLD) } else { theme::dim() };
    frame.render_widget(Paragraph::new(Span::styled(format!("  {text:<9}"), style)), Rect { x: inner.x, y, width: LABEL_W, height: 1 });
}

fn arrows(frame: &mut Frame, hits: &mut HitRegistry, x: u16, y: u16, glyphs: (&str, &str), (left, right): (Global, Global)) {
    frame.render_widget(Paragraph::new(Line::from(vec![Span::styled(glyphs.0, theme::accent()), Span::raw(" "), Span::styled(glyphs.1, theme::accent())])), Rect { x, y, width: 3, height: 1 });
    hits.click(Rect { x, y, width: 1, height: 1 }, left);
    hits.click(Rect { x: x + 2, y, width: 1, height: 1 }, right);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

fn fmt0(secs: u64) -> String {
    if secs == 0 { "0".into() } else { duration::format(secs) }
}
