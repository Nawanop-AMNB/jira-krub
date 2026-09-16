use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Single-line editor with a char-indexed cursor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInput {
    text: String,
    cursor: usize, // char index
}

impl TextInput {
    pub fn with(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.chars().count();
        Self { text, cursor }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
    pub fn set(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.len();
    }
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }
    fn len(&self) -> usize {
        self.text.chars().count()
    }
    fn byte_at(&self, char_idx: usize) -> usize {
        self.text.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(self.text.len())
    }
    pub fn insert(&mut self, c: char) {
        let b = self.byte_at(self.cursor);
        self.text.insert(b, c);
        self.cursor += 1;
    }
    pub fn insert_str(&mut self, s: &str) {
        // keep newlines (multi-line paste), drop other control chars
        let s: String = s.replace("\r\n", "\n").chars().filter(|c| *c == '\n' || !c.is_control()).collect();
        let b = self.byte_at(self.cursor);
        self.text.insert_str(b, &s);
        self.cursor += s.chars().count();
    }
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let b = self.byte_at(self.cursor - 1);
        self.text.remove(b);
        self.cursor -= 1;
    }
    pub fn delete(&mut self) {
        if self.cursor >= self.len() {
            return;
        }
        let b = self.byte_at(self.cursor);
        self.text.remove(b);
    }
    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }
    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.len());
    }
    pub fn home(&mut self) {
        self.cursor = 0;
    }
    pub fn end(&mut self) {
        self.cursor = self.len();
    }
    pub fn set_cursor_col(&mut self, col: usize) {
        self.cursor = col.min(self.len());
    }
    pub fn line_count(&self) -> usize {
        self.text.split('\n').count()
    }
    /// (line, column) of the cursor, in chars.
    fn cursor_line_col(&self) -> (usize, usize) {
        let before: String = self.text.chars().take(self.cursor).collect();
        let line = before.matches('\n').count();
        let col = before.rsplit('\n').next().map(|s| s.chars().count()).unwrap_or(0);
        (line, col)
    }

    /// Multi-line render: one text line per row, scrolled so the cursor row
    /// is visible. Used for the description field.
    pub fn render_lines(&self, frame: &mut Frame, area: Rect, style: Style, focused: bool, placeholder: &str) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let lines: Vec<&str> = self.text.split('\n').collect();
        let (cl, cc) = self.cursor_line_col();
        let h = area.height as usize;
        let top = cl.saturating_sub(h - 1);
        if self.text.is_empty() && !focused && !placeholder.is_empty() {
            frame.render_widget(Paragraph::new(Span::styled(placeholder, super::super::theme::dim())), Rect { height: 1, ..area });
            return;
        }
        for (row, line) in lines.iter().skip(top).take(h).enumerate() {
            let y = area.y + row as u16;
            let width = area.width as usize;
            let is_cursor_line = focused && top + row == cl;
            let offset = if is_cursor_line { cc.saturating_sub(width.saturating_sub(1)) } else { 0 };
            let visible: String = line.chars().skip(offset).take(width).collect();
            frame.render_widget(Paragraph::new(Span::styled(format!("{visible:<width$}"), style)), Rect { x: area.x, y, width: area.width, height: 1 });
            if is_cursor_line {
                let x = area.x + (cc - offset) as u16;
                frame.set_cursor_position(Position { x: x.min(area.x + area.width - 1), y });
            }
        }
    }

    /// Draw into `area`. Places the terminal cursor when `focused`.
    pub fn render(&self, frame: &mut Frame, area: Rect, style: Style, masked: bool, focused: bool, placeholder: &str) {
        let width = area.width as usize;
        if width == 0 {
            return;
        }
        let shown: String = if masked { "•".repeat(self.len()) } else { self.text.replace('\n', "⏎") };
        // horizontal scroll so the cursor stays visible
        let offset = self.cursor.saturating_sub(width.saturating_sub(1));
        let visible: String = shown.chars().skip(offset).take(width).collect();
        let line = if visible.is_empty() && !focused && !placeholder.is_empty() {
            Line::from(Span::styled(placeholder, super::super::theme::dim()))
        } else {
            Line::from(Span::styled(visible, style))
        };
        frame.render_widget(Paragraph::new(line).style(style), area);
        if focused {
            let x = area.x + (self.cursor - offset) as u16;
            frame.set_cursor_position(Position { x: x.min(area.x + area.width - 1), y: area.y });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_by_char() {
        let mut t = TextInput::with("héllo");
        t.left();
        t.left();
        t.insert('X');
        assert_eq!(t.text(), "hélXlo");
        t.backspace();
        t.backspace();
        assert_eq!(t.text(), "hélo");
        t.home();
        t.delete();
        assert_eq!(t.text(), "élo");
    }
}
