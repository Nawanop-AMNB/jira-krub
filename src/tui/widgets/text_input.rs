use super::text;
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_segmentation::UnicodeSegmentation;

/// Single-line editor with a char-indexed cursor.
///
/// Editing (`insert`, `backspace`, `delete`) works one code point at a time —
/// that is what macOS does for Thai, where a `⌫` peels off the combining mark
/// before the base character. Arrow keys move by grapheme cluster instead, so
/// one keypress crosses a whole syllable, and everything drawn on screen is
/// measured in terminal columns rather than chars.
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
    /// Terminal columns the whole text occupies (`\n` counts as one, matching
    /// the `⏎` it is drawn as).
    pub fn display_width(&self) -> usize {
        text::display_width(&self.text)
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
    /// Char indices of every grapheme-cluster boundary, `0` and the end included.
    fn boundaries(&self) -> Vec<usize> {
        let mut out = vec![0usize];
        let mut n = 0usize;
        for g in self.text.graphemes(true) {
            n += g.chars().count();
            out.push(n);
        }
        out
    }
    /// One grapheme cluster left — a whole Thai syllable, never a landing spot
    /// between a base character and its combining marks.
    pub fn left(&mut self) {
        self.cursor = self.boundaries().into_iter().rev().find(|&b| b < self.cursor).unwrap_or(0);
    }
    /// One grapheme cluster right.
    pub fn right(&mut self) {
        let end = self.len();
        self.cursor = self.boundaries().into_iter().find(|&b| b > self.cursor).unwrap_or(end);
    }
    pub fn home(&mut self) {
        self.cursor = 0;
    }
    pub fn end(&mut self) {
        self.cursor = self.len();
    }
    /// Place the cursor from a display column (e.g. a mouse click).
    ///
    /// `col` counts terminal columns, not chars. A click anywhere inside a
    /// grapheme lands at its *start*: column 1 of `日本` is the right half of
    /// `日`, so it yields char index 0, not 1. A column past the end of the
    /// text clamps to the end.
    pub fn set_cursor_display_col(&mut self, col: usize) {
        let mut used = 0usize;
        let mut idx = 0usize;
        for g in self.text.graphemes(true) {
            let w = text::display_width(g);
            if w > 0 && col < used + w {
                self.cursor = idx;
                return;
            }
            used += w;
            idx += g.chars().count();
        }
        self.cursor = idx;
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
    /// Display column of the cursor on its own line (line-local, not
    /// counted from the start of the whole text).
    pub(crate) fn cursor_display_col(&self) -> usize {
        let (cl, cc) = self.cursor_line_col();
        let line = self.text.split('\n').nth(cl).unwrap_or("");
        let before: String = line.chars().take(cc).collect();
        text::display_width(&before)
    }

    /// Multi-line render: one text line per row, scrolled so the cursor row
    /// is visible. Used for the description field.
    pub fn render_lines(&self, frame: &mut Frame, area: Rect, style: Style, focused: bool, placeholder: &str) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let lines: Vec<&str> = self.text.split('\n').collect();
        let (cl, _) = self.cursor_line_col();
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
            // cursor column on this line, in terminal columns
            let cur_col = if is_cursor_line { self.cursor_display_col() } else { 0 };
            let offset = if is_cursor_line { cur_col.saturating_sub(width.saturating_sub(1)) } else { 0 };
            let visible = text::window_by_cols(line, offset, width);
            frame.render_widget(Paragraph::new(Span::styled(visible, style)), Rect { x: area.x, y, width: area.width, height: 1 });
            if is_cursor_line {
                let x = area.x + (cur_col - offset) as u16;
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
        // Masking and the `⏎` substitution are both one shown char per source
        // char, so `cursor` still indexes `shown`.
        let shown: String = if masked { "•".repeat(self.len()) } else { self.text.replace('\n', "⏎") };
        let cur_col = text::display_width(&shown.chars().take(self.cursor).collect::<String>());
        // horizontal scroll, in columns, so the cursor stays visible
        let offset = cur_col.saturating_sub(width.saturating_sub(1));
        let line = if shown.is_empty() && !focused && !placeholder.is_empty() {
            Line::from(Span::styled(placeholder, super::super::theme::dim()))
        } else {
            Line::from(Span::styled(text::window_by_cols(&shown, offset, width), style))
        };
        frame.render_widget(Paragraph::new(line).style(style), area);
        if focused {
            let x = area.x + (cur_col - offset) as u16;
            frame.set_cursor_position(Position { x: x.min(area.x + area.width - 1), y: area.y });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::{Backend as _, TestBackend};

    impl TextInput {
        /// Char index of the cursor — test-only view of the internal position.
        fn cursor_char(&self) -> usize {
            self.cursor
        }
    }

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


    #[test]
    fn arrows_cross_a_whole_thai_syllable() {
        let mut t = TextInput::with("ที่"); // base + vowel + tone = one cluster
        assert_eq!(t.cursor_char(), 3);
        t.left();
        assert_eq!(t.cursor_char(), 0, "one keypress crosses all three code points");
        t.right();
        assert_eq!(t.cursor_char(), 3);
    }

    #[test]
    fn backspace_still_peels_one_code_point() {
        let mut t = TextInput::with("ที่");
        t.backspace();
        assert_eq!(t.text(), "ที");
        t.backspace();
        assert_eq!(t.text(), "ท");
    }

    #[test]
    fn click_column_maps_to_grapheme_start() {
        let mut t = TextInput::with("สวัสดี"); // clusters: ส | วั | ส | ดี
        t.set_cursor_display_col(2); // third column -> the second `ส`
        assert_eq!(t.cursor_char(), 3);
        t.set_cursor_display_col(0);
        assert_eq!(t.cursor_char(), 0);
        t.set_cursor_display_col(99);
        assert_eq!(t.cursor_char(), 6, "past the end clamps to the end");

        // A click inside a double-width glyph lands at its start: column 1 is
        // the right half of `日`, so the cursor goes before `日`, not after.
        let mut c = TextInput::with("日本");
        c.set_cursor_display_col(1);
        assert_eq!(c.cursor_char(), 0);
        c.set_cursor_display_col(2);
        assert_eq!(c.cursor_char(), 1);
    }

    #[test]
    fn ascii_click_column_is_the_char_index() {
        let mut t = TextInput::with("abcdef");
        t.set_cursor_display_col(3);
        assert_eq!(t.cursor_char(), 3);
        t.set_cursor_display_col(0);
        assert_eq!(t.cursor_char(), 0);
    }

    // --- contract test cases (T-numbers refer to the task's table) ---

    #[test]
    fn thai_cursor_display_col_ignores_combining_marks() {
        // T1
        let t = TextInput::with("สวัสดี");
        assert_eq!(t.cursor_display_col(), 4);
        assert_eq!(t.display_width(), 4);
    }

    #[test]
    fn cjk_cursor_display_col_counts_double_width() {
        // T2
        let mut t = TextInput::with("日本語");
        assert_eq!(t.cursor_display_col(), 6);
        t.left();
        assert_eq!(t.cursor_display_col(), 4);
    }

    #[test]
    fn left_right_move_by_grapheme() {
        // T3
        let mut t = TextInput::with("aที่b"); // clusters: a | ที่ | b
        let before = |t: &TextInput| -> String { t.text().chars().take(t.cursor_char()).collect() };
        t.left();
        assert_eq!(before(&t), "aที่");
        t.left();
        assert_eq!(before(&t), "a");
        t.right();
        t.insert('Y');
        assert_eq!(t.text(), "aที่Yb");
    }

    #[test]
    fn backspace_removes_one_code_point() {
        // T4
        let mut t = TextInput::with("ที่");
        t.backspace();
        assert_eq!(t.text(), "ที");
    }

    #[test]
    fn set_cursor_display_col_thai() {
        // T5
        let mut t = TextInput::with("สวัสดี");
        t.set_cursor_display_col(2);
        t.insert('X');
        assert_eq!(t.text(), "สวัXสดี");

        let mut t2 = TextInput::with("สวัสดี");
        t2.set_cursor_display_col(99);
        t2.insert('Z');
        assert!(t2.text().ends_with('Z'));
    }

    #[test]
    fn set_cursor_display_col_cjk_snaps_to_boundary() {
        // T6: a click on the right half of a wide glyph (column 1 of `日本`)
        // must not split it — the cursor lands at its start, not its middle.
        let mut t = TextInput::with("日本");
        t.set_cursor_display_col(1);
        t.insert('X');
        assert_eq!(t.text(), "X日本");

        let mut t2 = TextInput::with("日本");
        t2.set_cursor_display_col(2);
        t2.insert('X');
        assert_eq!(t2.text(), "日X本");
    }

    #[test]
    fn display_col_on_second_line() {
        // T7: line-local, not counted from the start of the whole text.
        let t = TextInput::with("ab\nสวัสดี");
        assert_eq!(t.cursor_display_col(), 4);
    }

    /// Renders `input` into a `width`-column, single-row `TestBackend` and
    /// reconstructs what a terminal would actually show: a wide glyph
    /// occupies two buffer cells (the second reset to a blank), so columns
    /// are walked by each cell's own display width rather than one at a
    /// time.
    fn render_row(input: &TextInput, width: u16, masked: bool) -> (String, Option<Position>) {
        let mut term = Terminal::new(TestBackend::new(width, 1)).expect("test backend");
        term.draw(|f| {
            let area = Rect { x: 0, y: 0, width, height: 1 };
            input.render(f, area, Style::new(), masked, true, "");
        })
        .expect("draw");
        let buf = term.backend().buffer().clone();
        let mut content = String::new();
        let mut x = 0u16;
        while x < width {
            let sym = buf[(x, 0)].symbol().to_string();
            let w = (text::display_width(&sym) as u16).max(1);
            content.push_str(&sym);
            x += w;
        }
        let pos = term.backend_mut().get_cursor_position().ok();
        (content, pos)
    }

    #[test]
    fn render_cjk_scrolls_to_keep_cursor_visible() {
        // T8
        let t = TextInput::with("日本語テキスト"); // 14 columns in a 6-column box
        let (row, pos) = render_row(&t, 6, false);
        // `キ` straddles the left edge, so its visible half is blanked out
        // rather than drawn as a broken glyph.
        assert!(!row.contains('キ'), "no half-drawn glyph: {row:?}");
        assert!(row.contains("スト"), "{row:?}");
        assert_eq!(text::display_width(&row), 6, "the row fills exactly six columns");
        // The cursor is one past the last character; with only 6 columns and
        // the last glyph (`ト`) ending exactly on column 5 (the box's last
        // column), that is where the terminal cursor is clamped to.
        assert_eq!(pos.map(|p| p.x), Some(5), "cursor sits on the last column");
    }

    #[test]
    fn render_thai_cursor_at_end_of_text() {
        // T9
        let t = TextInput::with("สวัสดี");
        let (_, pos) = render_row(&t, 10, false);
        assert_eq!(pos.map(|p| p.x), Some(4));
    }

    #[test]
    fn render_masked_cursor() {
        // T10: masked mode is one bullet per source char (not per grapheme
        // or per display column), so six chars draw six bullets.
        let t = TextInput::with("สวัสดี");
        let (row, pos) = render_row(&t, 10, true);
        assert!(row.starts_with("••••••"), "{row:?}");
        assert_eq!(pos.map(|p| p.x), Some(6));
    }

    #[test]
    fn narrow_viewport_scrolls_without_halving_a_wide_glyph() {
        let t = TextInput::with("日本語テキスト"); // 14 columns in a 6-column box
        let (row, pos) = render_row(&t, 6, false);
        assert_eq!(row, " スト ");
        assert_eq!(pos.map(|p| p.x), Some(5), "cursor sits on the last column");
    }

    #[test]
    fn ascii_rendering_is_unchanged() {
        let t = TextInput::with("hello");
        let (row, pos) = render_row(&t, 10, false);
        assert_eq!(row, "hello     ");
        assert_eq!(pos.map(|p| p.x), Some(5));

        // Scrolled: the cursor sits one column past the last visible
        // character (a blank cell), matching the pre-fix char-indexed
        // behaviour for ASCII (`cursor - (width - 1)`).
        let long = TextInput::with("abcdefghij");
        let (row, pos) = render_row(&long, 5, false);
        assert_eq!(row, "ghij ");
        assert_eq!(pos.map(|p| p.x), Some(4));
    }
}
