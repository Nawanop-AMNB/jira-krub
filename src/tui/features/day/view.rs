use super::model::{Action, Cell, Model, Pane};
use super::rows::{PrepareRows, Section, fmt0, prepare_rows, pushed_total, staged_total, ticket_rows};
use crate::domain::{DaySummary, duration};
use crate::tui::action::Action as Global;
use crate::tui::app::App;
use crate::tui::hit::{HitArea, HitRegistry};
use crate::tui::theme;
use crate::tui::view::Hint;
use crate::tui::widgets::truncate_to_width;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

pub fn view(frame: &mut Frame, app: &App, m: &Model, body: Rect, hits: &mut HitRegistry) -> Vec<Hint> {
    let rows = prepare_rows(app, m);
    let calendar = app.calendar();
    let summary = DaySummary::compute(m.date, app.today, &calendar, app.ledger.entries(), &app.remote.worklogs);

    let staged = staged_total(&rows);
    let need = match calendar.off_reason(m.date) {
        Some(reason) => format!("off · {reason}"),
        None => format!("need {}", fmt0(summary.remaining())),
    };
    let title = format!(
        " ◀ {} ▶ · pushed {} · {need}{} ",
        m.date.format("%a %d %b %Y"),
        fmt0(pushed_total(&rows)),
        if staged > 0 { format!(" · +{} staged", fmt0(staged)) } else { String::new() }
    );
    let block = Block::bordered().title(title).border_style(theme::accent());
    let inner = block.inner(body);
    frame.render_widget(block, body);
    // ◀ and ▶ in the title
    hits.click(Rect { x: body.x + 1, y: body.y, width: 3, height: 1 }, Global::Day(Action::PrevDay));
    let date_w = m.date.format("%a %d %b %Y").to_string().chars().count() as u16;
    hits.click(Rect { x: body.x + 4 + date_w + 1, y: body.y, width: 3, height: 1 }, Global::Day(Action::NextDay));

    let [left, sep, right] = Layout::horizontal([Constraint::Percentage(36), Constraint::Length(1), Constraint::Min(40)]).areas(inner);
    for y in sep.y..sep.y + sep.height {
        frame.render_widget(Paragraph::new(Span::styled("│", theme::dim())), Rect { x: sep.x, y, width: 1, height: 1 });
    }
    draw_tickets(frame, app, m, left, hits);
    draw_prepare(frame, app, m, &rows, right, hits);

    let mut hints = vec![Hint::new("p", "push", Global::Day(Action::PushDay)), Hint::new("[ ]", "day", Global::Nop), Hint::new("←→", "pane", Global::Nop)];
    if m.edit.is_some() {
        hints = vec![
            Hint::new("Enter", "next cell", Global::Day(Action::CellNext)),
            Hint::new("↑↓", "step 15m", Global::Nop),
            Hint::new("^J", "newline", Global::Day(Action::CellNewline)),
            Hint::new("Esc", "revert", Global::Day(Action::CellCancel)),
        ];
    } else if m.search_focused {
        hints.insert(0, Hint::new("Enter", "to list", Global::Day(Action::FocusTickets)));
        hints.insert(1, Hint::new("Esc", "clear", Global::Day(Action::SearchClear)));
    } else if m.pane == Pane::Tickets {
        hints.insert(0, Hint::new("Space", "stage", Global::Day(Action::QuickStage)));
        hints.insert(1, Hint::new("Enter", "form", Global::Day(Action::OpenForm)));
        hints.insert(2, Hint::new("/", "search", Global::Day(Action::FocusSearch)));
        hints.insert(3, Hint::new("w", "watch/unwatch", Global::Day(Action::ToggleWatch)));
        hints.push(Hint::new("Esc", "back", Global::Day(Action::Back)));
    } else {
        hints.insert(0, Hint::new("s", "start", Global::Day(Action::EditCell(Cell::Start))));
        hints.insert(1, Hint::new("d", "dur", Global::Day(Action::EditCell(Cell::Duration))));
        hints.insert(2, Hint::new("n", "desc", Global::Day(Action::EditCell(Cell::Title))));
        hints.insert(3, Hint::new("Enter", "edit row", Global::Day(Action::EditCell(Cell::Start))));
        hints.insert(4, Hint::new("⌫", "del", Global::Day(Action::Remove)));
        hints.push(Hint::new("Esc", "back", Global::Day(Action::Back)));
    }
    hints
}

fn draw_tickets(frame: &mut Frame, app: &App, m: &Model, area: Rect, hits: &mut HitRegistry) {
    // the search box is a row of this pane: while it has focus the list is not
    let focused = m.pane == Pane::Tickets && m.edit.is_none() && !m.search_focused;
    let header_style = if focused { theme::accent().add_modifier(Modifier::BOLD) } else { theme::dim() };
    frame.render_widget(Paragraph::new(Span::styled(" tickets", header_style)), Rect { x: area.x, y: area.y, width: area.width, height: 1 });

    // search line
    let sy = area.y + 1;
    frame.render_widget(Paragraph::new(Span::styled(" 🔍 ", if m.search_focused { theme::accent() } else { theme::dim() })), Rect { x: area.x, y: sy, width: 4, height: 1 });
    let srect = Rect { x: area.x + 4, y: sy, width: area.width.saturating_sub(5), height: 1 };
    m.search.render(frame, srect, Style::new(), false, m.search_focused, "type to filter, ≥2 chars searches jira");
    hits.click(srect, Global::Day(Action::FocusSearch));
    if m.s.loading {
        frame.render_widget(Paragraph::new(Span::styled("⟳", theme::warn())), Rect { x: area.x + area.width - 1, y: sy, width: 1, height: 1 });
    }

    let rows = ticket_rows(app, m);
    let list = Rect { x: area.x, y: area.y + 2, width: area.width, height: area.height.saturating_sub(2) };
    hits.add(HitArea {
        rect: list,
        scroll_up: Some(Global::Day(Action::Up)),
        scroll_down: Some(Global::Day(Action::Down)),
        ..Default::default()
    });

    if rows.is_empty() {
        let msg = if m.query().is_empty() { "no tickets yet — r to sync, / to search" } else if m.s.loading { "searching…" } else { "no match" };
        frame.render_widget(Paragraph::new(Span::styled(format!("   {msg}"), theme::dim())), Rect { x: list.x, y: list.y + 1, width: list.width, height: 1 });
        return;
    }

    // Build display lines: header per section change + rows. Track which line is selected for scrolling.
    struct L {
        text: Line<'static>,
        row: Option<usize>,
    }
    let mut lines: Vec<L> = Vec::new();
    let mut last: Option<Section> = None;
    for (i, r) in rows.iter().enumerate() {
        if last != Some(r.section) {
            let count = rows.iter().filter(|x| x.section == r.section).count();
            let label = match r.section {
                Section::Jira => format!("─ {} ({count}) ", r.section.label()),
                s => format!("─ {} ", s.label()),
            };
            let pad = (list.width as usize).saturating_sub(label.chars().count() + 1);
            lines.push(L { text: Line::from(Span::styled(format!(" {label}{}", "─".repeat(pad)), theme::dim())), row: None });
            last = Some(r.section);
        }
        let selected = i == m.ticket_sel;
        let marker = if selected && focused { "▶ " } else { "  " };
        let star = if r.watched { "*" } else { " " };
        let key = format!("{:<9}", r.issue.key);
        let summary_w = (list.width as usize).saturating_sub(2 + 1 + 1 + 9 + 1);
        let summary = truncate_to_width(&r.issue.summary, summary_w);
        let key_style = match r.section {
            Section::History => theme::dim(),
            _ => theme::issue_key(),
        };
        let mut line = Line::from(vec![
            Span::raw(marker),
            Span::styled(star.to_string(), theme::watched()),
            Span::raw(" "),
            Span::styled(key, key_style),
            Span::raw(" "),
            Span::styled(summary, if r.section == Section::History { theme::dim() } else { Style::new() }),
        ]);
        if selected {
            line = line.style(if focused { theme::selected() } else { theme::bold() });
        }
        lines.push(L { text: line, row: Some(i) });
    }

    // scroll so the selected row is visible
    let sel_line = lines.iter().position(|l| l.row == Some(m.ticket_sel)).unwrap_or(0);
    let h = list.height as usize;
    let offset = if h == 0 { 0 } else { sel_line.saturating_sub(h.saturating_sub(1)).min(sel_line) };
    let offset = if sel_line >= offset + h { sel_line + 1 - h } else { offset };
    for (k, l) in lines.iter().skip(offset).take(h).enumerate() {
        let rect = Rect { x: list.x, y: list.y + k as u16, width: list.width, height: 1 };
        frame.render_widget(Paragraph::new(l.text.clone()), rect);
        if let Some(i) = l.row {
            hits.add(HitArea {
                rect,
                click: Some(Global::Day(Action::SelectTicket(i))),
                double: Some(Global::Day(Action::StageKey(rows[i].issue.key.clone()))),
                drag: Some(rows[i].issue.key.clone()),
                scroll_up: Some(Global::Day(Action::Up)),
                scroll_down: Some(Global::Day(Action::Down)),
                ..Default::default()
            });
        }
    }
}

// column layout for the prepare pane
const C_MARK: u16 = 2;
const C_START: u16 = 5;
const C_DUR: u16 = 7;
const C_KEY: u16 = 10;
/// Issue summary column (read-only), truncated. Shrinks on narrow panes.
const C_SUM_MAX: u16 = 22;
const C_SUM_MIN: u16 = 8;

fn summary_width(pane_width: u16) -> u16 {
    let fixed = C_MARK + C_START + C_DUR + C_KEY + GAP * 4 + 2;
    let spare = pane_width.saturating_sub(fixed + 14);
    (spare / 2).clamp(C_SUM_MIN, C_SUM_MAX)
}
const GAP: u16 = 2;

fn draw_prepare(frame: &mut Frame, app: &App, m: &Model, rows: &PrepareRows, area: Rect, hits: &mut HitRegistry) {
    let focused = m.pane == Pane::Prepare;
    let header_style = if focused { theme::accent().add_modifier(Modifier::BOLD) } else { theme::dim() };
    frame.render_widget(Paragraph::new(Span::styled(" prepare logwork", header_style)), Rect { x: area.x, y: area.y, width: area.width, height: 1 });
    hits.add(HitArea { rect: area, drop_target: true, click: Some(Global::Day(Action::FocusPrepare)), scroll_up: Some(Global::Day(Action::Up)), scroll_down: Some(Global::Day(Action::Down)), ..Default::default() });

    let x_start0 = area.x + C_MARK;
    let c_sum = summary_width(area.width);
    let y = area.y + 1;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<w$}", "start", w = (C_START + GAP) as usize), theme::dim()),
            Span::styled(format!("{:<w$}", "dur", w = (C_DUR + GAP) as usize), theme::dim()),
            Span::styled(format!("{:<w$}", "ticket", w = (C_KEY + GAP) as usize), theme::dim()),
            Span::styled(format!("{:<w$}", "title", w = (c_sum + GAP) as usize), theme::dim()),
            Span::styled("description", theme::dim()),
        ])),
        Rect { x: area.x, y, width: area.width, height: 1 },
    );

    let mut y = y + 1;
    let bottom = area.y + area.height;

    if rows.pending_len == 0 {
        if y + 1 < bottom {
            frame.render_widget(Paragraph::new(Span::styled("   nothing pending", theme::dim())), Rect { x: area.x, y, width: area.width, height: 1 });
            frame.render_widget(Paragraph::new(Span::styled("   Space or double-click a ticket to stage it", theme::dim())), Rect { x: area.x, y: y + 1, width: area.width, height: 1 });
        }
        y += 3;
    }

    for (i, row) in rows.rows.iter().enumerate() {
        if y >= bottom {
            break;
        }
        // divider between pending rows and rows already in Jira
        if i == rows.pending_len && i > 0 {
            let label = " ─ in jira ";
            let pad = (area.width as usize).saturating_sub(label.chars().count());
            frame.render_widget(Paragraph::new(Span::styled(format!("{label}{}", "─".repeat(pad)), theme::dim())), Rect { x: area.x, y, width: area.width, height: 1 });
            y += 1;
            if y >= bottom {
                break;
            }
        }
        let selected = i == m.prepare_sel && focused;
        let local = row.local();
        let editing = local.and_then(|e| m.edit.as_ref().filter(|ed| ed.id == e.id));
        let deleted = row.is_deleted();
        let in_jira = i >= rows.pending_len;
        let overlaps = local.is_some_and(|e| rows.rows.iter().filter_map(|o| o.local()).any(|o| o.overlaps(e)));

        let row_rect = Rect { x: area.x, y, width: area.width, height: 1 };
        hits.add(HitArea { rect: row_rect, click: Some(Global::Day(Action::SelectPrepare(i))), double: Some(Global::Day(Action::EditCellOf(i, Cell::Start))), scroll_up: Some(Global::Day(Action::Up)), scroll_down: Some(Global::Day(Action::Down)), ..Default::default() });
        if selected && editing.is_none() {
            frame.render_widget(Paragraph::new("").style(theme::selected()), row_rect);
        }

        // state marker
        let (mark, mark_style) = match local {
            Some(e) if e.is_failed() => ("!", theme::bad()),
            Some(e) if e.is_deleted() => ("✗", theme::bad()),
            Some(e) if e.is_modified() => ("~", theme::warn()),
            Some(e) if e.needs_push() => ("+", theme::warn()),
            _ => (" ", theme::dim()),
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::raw(if selected { "▶" } else { " " }), Span::styled(mark, mark_style)])),
            Rect { x: area.x, y, width: C_MARK, height: 1 },
        );

        // Column widths for this row: an editing cell grows to fit its text,
        // the cursor and the ▲▼ steppers; later columns shift right.
        let edit_w = |cell: Cell, base: u16| -> u16 {
            match editing.filter(|ed| ed.cell == cell) {
                Some(ed) => base.max(ed.input.display_width() as u16 + 1) + if cell == Cell::Title { 0 } else { 2 },
                None => base,
            }
        };
        let start_w = edit_w(Cell::Start, C_START);
        let dur_w = edit_w(Cell::Duration, C_DUR);
        let x_start = x_start0;
        let x_dur = x_start + start_w + GAP;
        let x_key = x_dur + dur_w + GAP;
        let x_sum = x_key + C_KEY + GAP;
        let x_title = x_sum + c_sum + GAP;
        let title_w = (area.x + area.width).saturating_sub(x_title + if in_jira { 2 } else { 0 });

        let title_txt = row.title().replace('\n', " ⏎ ");
        let cells = [
            (Cell::Start, Rect { x: x_start, y, width: start_w, height: 1 }, row.start().to_string()),
            (Cell::Duration, Rect { x: x_dur, y, width: dur_w, height: 1 }, duration::format(row.seconds())),
            (Cell::Title, Rect { x: x_title, y, width: title_w, height: 1 }, if title_txt.trim().is_empty() { "(no description)".into() } else { title_txt }),
        ];
        for (cell, rect, text) in cells {
            match editing.filter(|ed| ed.cell == cell) {
                Some(ed) => {
                    let style = if ed.error.is_some() { theme::editing_error() } else { theme::editing() };
                    let input_rect = if cell == Cell::Title { rect } else { Rect { width: rect.width.saturating_sub(2), ..rect } };
                    ed.input.render(frame, input_rect, style, false, true, "");
                    hits.add(HitArea { rect: input_rect, click_at: Some(Box::new(|col| Global::Day(Action::CellCursor(col)))), ..Default::default() });
                    if cell != Cell::Title {
                        let sx = input_rect.x + input_rect.width;
                        frame.render_widget(Paragraph::new(Line::from(vec![Span::styled("▲", theme::accent()), Span::styled("▼", theme::accent())])), Rect { x: sx, y, width: 2, height: 1 });
                        hits.click(Rect { x: sx, y, width: 1, height: 1 }, Global::Day(Action::CellStep(15)));
                        hits.click(Rect { x: sx + 1, y, width: 1, height: 1 }, Global::Day(Action::CellStep(-15)));
                    }
                }
                None => {
                    let mut style = if deleted { theme::dim().add_modifier(Modifier::CROSSED_OUT) } else { theme::editable() };
                    if in_jira && !deleted {
                        style = style.add_modifier(Modifier::DIM);
                    }
                    if cell == Cell::Title && text == "(no description)" {
                        style = style.add_modifier(Modifier::DIM);
                    }
                    if selected {
                        style = style.add_modifier(Modifier::REVERSED);
                    }
                    frame.render_widget(Paragraph::new(Span::styled(truncate_to_width(&text, rect.width as usize), style)), rect);
                    hits.add(HitArea {
                        rect,
                        click: Some(Global::Day(Action::EditCellOf(i, cell))),
                        scroll_up: Some(Global::Day(Action::Up)),
                        scroll_down: Some(Global::Day(Action::Down)),
                        ..Default::default()
                    });
                }
            }
        }
        let mut key_style = if deleted { theme::dim().add_modifier(Modifier::CROSSED_OUT) } else { theme::issue_key() };
        if in_jira {
            key_style = key_style.add_modifier(Modifier::DIM);
        }
        if selected {
            key_style = key_style.add_modifier(Modifier::REVERSED);
        }
        frame.render_widget(Paragraph::new(Span::styled(format!("{:<w$}", row.key().to_string(), w = C_KEY as usize), key_style)), Rect { x: x_key, y, width: C_KEY, height: 1 });
        // issue title (read-only, from Jira)
        let summary = app.remote.issue(row.key()).map(|i| i.summary.clone()).unwrap_or_default();
        let mut sum_style = if deleted { theme::dim().add_modifier(Modifier::CROSSED_OUT) } else { theme::dim() };
        if selected {
            sum_style = sum_style.add_modifier(Modifier::REVERSED);
        }
        frame.render_widget(Paragraph::new(Span::styled(truncate_to_width(&summary, c_sum as usize), sum_style)), Rect { x: x_sum, y, width: c_sum, height: 1 });
        if overlaps && !deleted {
            frame.render_widget(Paragraph::new(Span::styled("!", theme::warn())), Rect { x: x_key + C_KEY, y, width: 1, height: 1 });
        }
        if in_jira {
            frame.render_widget(Paragraph::new(Span::styled("✓", theme::good())), Rect { x: area.x + area.width - 1, y, width: 1, height: 1 });
        }
        if let Some(crate::domain::EntryState::Failed { error, .. }) = local.map(|e| &e.state) {
            y += 1;
            if y < bottom {
                frame.render_widget(
                    Paragraph::new(Span::styled(format!("      ↳ {}", truncate_to_width(error, area.width.saturating_sub(8) as usize)), theme::bad())),
                    Rect { x: area.x, y, width: area.width, height: 1 },
                );
            }
        }
        y += 1;
    }
    if let Some(ed) = &m.edit
        && let Some(err) = &ed.error
        && y < bottom
    {
        frame.render_widget(Paragraph::new(Span::styled(format!("   ✗ {err}"), theme::bad())), Rect { x: area.x, y, width: area.width, height: 1 });
    }
}

