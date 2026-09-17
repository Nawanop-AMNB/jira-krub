use crate::tui::action::Action;
use crate::tui::hit::HitRegistry;
use crate::tui::theme;
use crate::tui::widgets::text::display_width;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

/// Columns between two chips.
const GAP: u16 = 2;

/// One reusable tab-bar row: `label` chips (`active` = `theme::title()`, others
/// `theme::dim()`), each rendered as ` label ` padded with a 2-column gap
/// between chips, each clickable via `hits`. Returns the x-column just past
/// the last chip so the caller can draw more on the same row (arrows, a
/// right-aligned hint or status).
pub fn draw(frame: &mut Frame, area: Rect, tabs: &[(&str, bool, Action)], hits: &mut HitRegistry) -> u16 {
    let mut x = area.x;
    for (label, active, action) in tabs {
        let text = format!(" {label} ");
        let w = display_width(&text) as u16;
        let style = if *active { theme::title() } else { theme::dim() };
        let rect = Rect { x, y: area.y, width: w, height: 1 };
        frame.render_widget(Paragraph::new(Span::styled(text, style)), rect);
        hits.click(rect, action.clone());
        x += w + GAP;
    }
    x
}
