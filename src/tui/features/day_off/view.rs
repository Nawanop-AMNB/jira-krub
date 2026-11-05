use super::model::{Action, Focus, Model};
use crate::tui::action::Action as Global;
use crate::tui::hit::HitRegistry;
use crate::tui::theme;
use crate::tui::view::{Hint, centered};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Clear, Paragraph};

const WIDTH: u16 = 48;
/// padding, date, name, padding, error line, buttons — inside the border
const HEIGHT: u16 = 6 + 2;
const LABEL_W: u16 = 8;
/// Blank columns between `[ Save ]` and `[ Cancel ]`.
const BUTTON_GAP: u16 = 2;

pub fn view(frame: &mut Frame, m: &Model, body: Rect, hits: &mut HitRegistry) -> Vec<Hint> {
    let area = centered(WIDTH, HEIGHT, body);
    frame.render_widget(Clear, area);
    let title = if m.is_edit() { " edit day off " } else { " day off " };
    let block = Block::bordered().title(title).border_style(theme::accent());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    hits.click(area, Global::Nop); // swallow clicks on the dialog background

    let fx = inner.x + LABEL_W;
    let fw = inner.width.saturating_sub(LABEL_W + 1);
    let mut y = inner.y + 1;
    for (field, label, input, placeholder) in [(Focus::Date, "date", &m.date, "YYYY-MM-DD"), (Focus::Name, "name", &m.name, "day off")] {
        let focused = m.focus == field;
        let label_style = if focused { theme::accent().add_modifier(Modifier::BOLD) } else { theme::dim() };
        frame.render_widget(Paragraph::new(Span::styled(format!("  {label}"), label_style)), Rect { x: inner.x, y, width: LABEL_W, height: 1 });
        let rect = Rect { x: fx, y, width: fw, height: 1 };
        input.render(frame, rect, if focused { theme::editing() } else { Style::new() }, false, focused, placeholder);
        hits.click(rect, Global::DayOff(Action::Focus(field)));
        y += 1;
    }
    if let Some(e) = &m.error {
        let rect = Rect { x: inner.x, y: y + 1, width: inner.width, height: 1 };
        frame.render_widget(Paragraph::new(Span::styled(format!("  ✗ {e}"), theme::bad())), rect);
    }

    let by = inner.y + inner.height - 1;
    let save = Button::new("Save", theme::accent(), m.focus == Focus::Save, Action::Save);
    let cancel = Button::new("Cancel", theme::accent(), m.focus == Focus::Cancel, Action::Cancel);
    let right_w = save.width() + BUTTON_GAP + cancel.width();
    let bx = inner.right().saturating_sub(right_w + 1);
    save.draw(frame, bx, by, hits);
    cancel.draw(frame, bx + save.width() + BUTTON_GAP, by, hits);
    if m.is_edit() {
        Button::new("Delete", theme::bad(), m.focus == Focus::Delete, Action::Remove).draw(frame, inner.x + 2, by, hits);
    }

    vec![
        Hint::new("↑↓←→", "move", Global::Nop),
        Hint::new("Enter", "press", Global::DayOff(Action::Enter)),
        Hint::new("Esc", "close", Global::DayOff(Action::Cancel)),
    ]
}

/// A `[ label ]` button; the focused one is drawn reversed.
struct Button {
    text: String,
    style: Style,
    action: Action,
}

impl Button {
    fn new(label: &str, style: Style, focused: bool, action: Action) -> Self {
        let style = if focused { style.add_modifier(Modifier::REVERSED) } else { style };
        Self { text: format!("[ {label} ]"), style, action }
    }

    fn width(&self) -> u16 {
        self.text.chars().count() as u16
    }

    fn draw(&self, frame: &mut Frame, x: u16, y: u16, hits: &mut HitRegistry) {
        let rect = Rect { x, y, width: self.width(), height: 1 };
        frame.render_widget(Paragraph::new(Span::styled(self.text.as_str(), self.style)), rect);
        hits.click(rect, Global::DayOff(self.action.clone()));
    }
}
