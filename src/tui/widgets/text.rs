use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Terminal columns `s` occupies, with `\n` counted as 1 (it is drawn as `⏎`).
pub fn display_width(s: &str) -> usize {
    s.split('\n').map(UnicodeWidthStr::width).sum::<usize>() + s.matches('\n').count()
}

/// Shorten `s` so it occupies at most `max_cols` terminal columns.
///
/// Thai combining marks are zero-width and CJK glyphs are two columns wide, so
/// the cut is made on grapheme-cluster boundaries and measured in columns — a
/// wide glyph is dropped rather than halved.
pub fn truncate_to_width(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    if display_width(s) <= max_cols {
        return s.to_string();
    }
    let budget = max_cols - 1; // one column for the ellipsis
    let mut used = 0usize;
    let mut out = String::new();
    for g in s.graphemes(true) {
        let w = display_width(g);
        if used + w > budget {
            break;
        }
        used += w;
        out.push_str(g);
    }
    out.push('…');
    out
}

/// Right-pad `s` with spaces to `cols` terminal columns. Never truncates.
pub fn pad_to_width(s: &str, cols: usize) -> String {
    let w = display_width(s);
    let mut out = s.to_string();
    out.push_str(&" ".repeat(cols.saturating_sub(w)));
    out
}

/// The slice of `s` visible in a `width`-column viewport scrolled `offset`
/// columns to the right, padded with spaces to exactly `width` columns.
///
/// A wide grapheme that would be halved by either edge is replaced by spaces
/// for the columns it would have covered, so every later column stays put.
pub fn window_by_cols(s: &str, offset: usize, width: usize) -> String {
    let mut out = String::new();
    let mut col = 0usize; // column of the next grapheme within `s`
    let mut used = 0usize; // columns already emitted
    for g in s.graphemes(true) {
        if used >= width {
            break;
        }
        let w = display_width(g);
        if col + w <= offset {
            col += w; // fully scrolled off to the left
            continue;
        }
        if col < offset {
            // straddles the left edge: blank out the half that shows
            let visible = (col + w - offset).min(width - used);
            out.push_str(&" ".repeat(visible));
            used += visible;
        } else if used + w > width {
            break; // would be halved by the right edge: drop it
        } else {
            out.push_str(g);
            used += w;
        }
        col += w;
    }
    out.push_str(&" ".repeat(width - used));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_width_counts_columns_not_chars() {
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width("สวัสดี"), 4); // 6 chars, 2 of them combining
        assert_eq!(display_width("日本"), 4); // 2 chars, 2 columns each
        assert_eq!(display_width("a\nb"), 3); // `\n` renders as `⏎`
        assert_eq!(display_width("⏎"), 1, "the `\\n` substitute must be one column");
    }

    #[test]
    fn truncate_leaves_short_ascii_alone() {
        assert_eq!(truncate_to_width("hello", 10), "hello");
        assert_eq!(truncate_to_width("hello", 5), "hello");
        assert_eq!(truncate_to_width("hello", 4), "hel…");
    }

    #[test]
    fn truncate_thai_by_columns() {
        let out = truncate_to_width("สวัสดีครับ", 4);
        assert!(out.ends_with('…'), "{out:?}");
        assert!(display_width(&out) <= 4, "{out:?} is {} cols", display_width(&out));
        assert_eq!(out, "สวัส…");
    }

    #[test]
    fn truncate_cjk_never_halves_a_wide_char() {
        let out = truncate_to_width("日本語テキスト", 5);
        assert!(display_width(&out) <= 5, "{out:?} is {} cols", display_width(&out));
        assert_eq!(out, "日本…"); // 4 columns: a third glyph would need 6

        // With only 4 columns of budget, even `日本` (4 columns) would leave
        // no room for the ellipsis, so only one glyph fits.
        let out4 = truncate_to_width("日本語テキスト", 4);
        assert_eq!(out4, "日…");
        assert_eq!(display_width(&out4), 3);
    }

    #[test]
    fn truncate_to_zero_is_empty() {
        assert_eq!(truncate_to_width("anything", 0), "");
        assert_eq!(truncate_to_width("", 0), "");
        assert_eq!(truncate_to_width("abc", 1), "…");
    }

    #[test]
    fn pad_uses_columns() {
        assert_eq!(pad_to_width("日本", 6), "日本  ");
        assert_eq!(pad_to_width("สวัสดี", 6), "สวัสดี  ");
        assert_eq!(pad_to_width("wide", 2), "wide");
    }

    #[test]
    fn window_drops_rather_than_halves() {
        // 日本語 = columns 0..6
        assert_eq!(window_by_cols("日本語", 0, 5), "日本 ");
        // offset 1 splits 日: its right half becomes a space
        assert_eq!(window_by_cols("日本語", 1, 4), " 本 ");
        assert_eq!(window_by_cols("abc", 1, 4), "bc  ");
        assert_eq!(window_by_cols("", 0, 3), "   ");
    }
}
