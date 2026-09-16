use super::model::{Action, Focus, Model, TestState};
use crate::tui::action::Action as Global;
use crate::tui::app::App;
use crate::tui::hit::{HitArea, HitRegistry};
use crate::tui::theme;
use crate::tui::view::{Hint, button, centered};
use crate::tui::widgets::TextInput;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

const TOKEN_URL: &str = "id.atlassian.com/manage-profile/security/api-tokens";

pub fn view(frame: &mut Frame, app: &App, m: &Model, body: Rect, hits: &mut HitRegistry) -> Vec<Hint> {
    let area = centered(70.min(body.width), 15.min(body.height), body);
    let block = Block::bordered().title(" jira-krub · connect ").border_style(theme::accent());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut y = inner.y;
    if let Some(b) = &m.banner {
        frame.render_widget(Paragraph::new(Span::styled(format!("  {b}"), theme::bad())), Rect { x: inner.x, y, width: inner.width, height: 1 });
        y += 1;
    } else {
        y += 1;
    }

    let ed = m.editing;
    y = field(frame, hits, inner, y, "Jira site", &m.site, (m.focus == Focus::Site, ed), Focus::Site, false, "https://company.atlassian.net", m.site_err.as_deref());
    hint_line(frame, inner, y, "e.g. https://company.atlassian.net");
    y += 1;
    y = field(frame, hits, inner, y, "Email", &m.email, (m.focus == Focus::Email, ed), Focus::Email, false, "you@company.com", m.email_err.as_deref());
    let token_placeholder = if m.token_kept { "•••••••••••••••• (kept, type to replace)" } else { "paste token" };
    y = field(frame, hits, inner, y, "API token", &m.token, (m.focus == Focus::Token, ed), Focus::Token, true, token_placeholder, m.token_err.as_deref());
    hint_line(frame, inner, y, TOKEN_URL);
    y += 2;

    let (text, style) = match &m.test {
        TestState::Idle => (String::from("  not tested yet"), theme::dim()),
        TestState::Testing => (String::from("  ⟳ testing…"), theme::warn()),
        TestState::Ok { display_name, .. } => (format!("  ✓ signed in as {display_name}"), theme::good()),
        TestState::Err(e) => (format!("  ✗ {e}"), theme::bad()),
    };
    frame.render_widget(Paragraph::new(Span::styled(text, style)), Rect { x: inner.x, y, width: inner.width, height: 1 });
    y += 2;

    let mut x = inner.x + 14;
    x += button(frame, x, y, "Test connection", m.focus == Focus::TestBtn, Global::Connect(Action::Test), hits) + 3;
    x += button(frame, x, y, "Save & start", m.focus == Focus::SaveBtn, Global::Connect(Action::Save), hits) + 3;
    let quit_label = if app.gateway.is_some() { "Back" } else { "Quit" };
    button(frame, x, y, quit_label, m.focus == Focus::QuitBtn, Global::Connect(Action::Quit), hits);
    y += 2;
    if y < inner.y + inner.height {
        hint_line(frame, inner, y, &format!("saved to {} (mode 600)", app.deps.config_store.location()));
    }

    if m.editing {
        return vec![
            Hint::new("Enter", "next field", Global::Connect(Action::Commit(1))),
            Hint::new("Esc", "revert", Global::Connect(Action::Revert)),
        ];
    }
    vec![
        Hint::new("↑↓", "field", Global::Connect(Action::FocusNext)),
        Hint::new("Enter", "edit / activate", Global::Connect(Action::Activate)),
        Hint::new("Esc", if app.gateway.is_some() { "back" } else { "quit" }, Global::Connect(Action::Quit)),
    ]
}

#[allow(clippy::too_many_arguments)]
fn field(
    frame: &mut Frame,
    hits: &mut HitRegistry,
    inner: Rect,
    y: u16,
    label: &str,
    input: &TextInput,
    (focused, editing): (bool, bool),
    focus: Focus,
    masked: bool,
    placeholder: &str,
    err: Option<&str>,
) -> u16 {
    let label_w = 13u16;
    let label_style = if focused { theme::accent() } else { theme::dim() };
    frame.render_widget(
        Paragraph::new(Span::styled(format!("  {label:<11}"), label_style)),
        Rect { x: inner.x, y, width: label_w, height: 1 },
    );
    let box_w = inner.width.saturating_sub(label_w + 3);
    let box_rect = Rect { x: inner.x + label_w, y, width: box_w, height: 1 };
    frame.render_widget(Paragraph::new("["), Rect { x: box_rect.x - 1, y, width: 1, height: 1 });
    frame.render_widget(Paragraph::new("]"), Rect { x: box_rect.x + box_w, y, width: 1, height: 1 });
    let open = focused && editing;
    let style = if err.is_some() { theme::bad() } else if open { theme::editing() } else { ratatui::style::Style::new() };
    input.render(frame, box_rect, style, masked, open, placeholder);
    hits.add(HitArea {
        rect: box_rect,
        click: Some(Global::Connect(Action::Focus(focus))),
        ..Default::default()
    });
    if let Some(e) = err {
        let ex = box_rect.x + box_w + 2;
        if ex < inner.x + inner.width {
            frame.render_widget(
                Paragraph::new(Span::styled(e.to_string(), theme::bad())),
                Rect { x: ex, y, width: (inner.x + inner.width).saturating_sub(ex), height: 1 },
            );
        }
    }
    y + 1
}

fn hint_line(frame: &mut Frame, inner: Rect, y: u16, text: &str) {
    if y >= inner.y + inner.height {
        return;
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(format!("{:13}{text}", ""), theme::dim()))),
        Rect { x: inner.x, y, width: inner.width, height: 1 },
    );
}
