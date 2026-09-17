use super::action::Action;
use super::app::{App, Overlay, Screen, StatusKind};
use super::features::{connect, day, entry_form, settings, week};
use super::hit::HitRegistry;
use super::theme;
use super::widgets::text::display_width;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

/// A footer hint: key label, description, action when clicked.
pub struct Hint {
    pub key: &'static str,
    pub label: &'static str,
    pub action: Action,
}

impl Hint {
    pub fn new(key: &'static str, label: &'static str, action: Action) -> Self {
        Self { key, label, action }
    }
}

pub fn draw(frame: &mut Frame, app: &App, hits: &mut HitRegistry) {
    let [body, footer] = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());

    let hints = match &app.screen {
        Screen::Connect(m) => connect::view(frame, app, m, body, hits),
        Screen::Settings(m) => settings::view(frame, app, m, body, hits),
        Screen::Week(m) => week::view(frame, app, m, body, hits),
        Screen::Day(m) => day::view(frame, app, m, body, hits),
    };

    let hints = match &app.overlay {
        Some(Overlay::Form(m)) => entry_form::view(frame, app, m, body, hits),
        Some(Overlay::Confirm(c)) => {
            draw_confirm(frame, &c.text, body, hits);
            vec![Hint::new("y", "yes", Action::ConfirmYes), Hint::new("n", "no", Action::ConfirmNo)]
        }
        None => hints,
    };

    draw_footer(frame, app, &hints, footer, hits);
}

fn draw_footer(frame: &mut Frame, app: &App, hints: &[Hint], area: Rect, hits: &mut HitRegistry) {
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    let mut x = area.x + 1;
    for h in hints {
        let text = format!("{} {}", h.key, h.label);
        let w = text.chars().count() as u16;
        hits.click(Rect { x, y: area.y, width: w, height: 1 }, h.action.clone());
        spans.push(Span::styled(h.key.to_string(), theme::key_hint()));
        spans.push(Span::styled(format!(" {}", h.label), theme::dim()));
        spans.push(Span::styled(" · ", theme::dim()));
        x += w + 3;
    }
    if let Some(s) = &app.status {
        let style = match s.kind {
            StatusKind::Info => theme::dim(),
            StatusKind::Error => theme::bad(),
            StatusKind::Busy => theme::warn(),
        };
        spans.push(Span::styled(format!("  {}", s.text), style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_confirm(frame: &mut Frame, text: &str, body: Rect, hits: &mut HitRegistry) {
    let width = (display_width(text) as u16 + 6).clamp(30, body.width.saturating_sub(4).max(30));
    let area = centered(width, 5, body);
    frame.render_widget(Clear, area);
    let block = Block::bordered().title(" confirm ").border_style(theme::warn());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), inner);
    // whole dialog: click = yes on the left half, no on the right half is too clever; use footer hints
    hits.click(area, Action::Nop);
}

pub fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h }
}

/// `[label]` button at a position; registers the click. Returns width used.
pub fn button(frame: &mut Frame, x: u16, y: u16, label: &str, focused: bool, action: Action, hits: &mut HitRegistry) -> u16 {
    let text = format!("[ {label} ]");
    let w = text.chars().count() as u16;
    let rect = Rect { x, y, width: w, height: 1 };
    let style = if focused { theme::title() } else { theme::accent() };
    frame.render_widget(Paragraph::new(Span::styled(text, style)), rect);
    hits.click(rect, action);
    w
}
