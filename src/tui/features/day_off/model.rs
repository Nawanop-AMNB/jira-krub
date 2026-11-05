use crate::application::config::Holiday;
use crate::tui::widgets::TextInput;
use chrono::{Datelike, NaiveDate};

/// What a day off is called when the user leaves the name blank.
pub const DEFAULT_NAME: &str = "day off";

/// Everything keyboard focus can rest on, in Tab order. `Delete` only exists
/// when editing an existing day off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Date,
    Name,
    Delete,
    Save,
    Cancel,
}

const ADD_ORDER: [Focus; 4] = [Focus::Date, Focus::Name, Focus::Save, Focus::Cancel];
const EDIT_ORDER: [Focus; 5] = [Focus::Date, Focus::Name, Focus::Delete, Focus::Save, Focus::Cancel];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// Year a short date (`11-6`) is read in: the selected day's.
    pub year: i32,
    /// The date the day off had when the modal opened; `Some` means edit.
    pub original: Option<NaiveDate>,
    pub date: TextInput,
    pub name: TextInput,
    pub focus: Focus,
    pub error: Option<String>,
}

impl Model {
    /// `existing` is the day off already on `date`, which switches to edit mode.
    pub fn new(date: NaiveDate, existing: Option<&Holiday>) -> Self {
        Self {
            year: date.year(),
            original: existing.map(|h| h.date),
            date: TextInput::with(date.format("%Y-%m-%d").to_string()),
            name: TextInput::with(existing.map(|h| h.name.clone()).unwrap_or_default()),
            focus: Focus::Name,
            error: None,
        }
    }

    pub fn is_edit(&self) -> bool {
        self.original.is_some()
    }

    /// Every focus target, in Tab order.
    fn order(&self) -> &'static [Focus] {
        if self.is_edit() { &EDIT_ORDER } else { &ADD_ORDER }
    }

    /// The buttons, left to right.
    fn buttons(&self) -> &'static [Focus] {
        &self.order()[2..]
    }

    fn on_button(&self) -> bool {
        !matches!(self.focus, Focus::Date | Focus::Name)
    }

    /// Run `f` on the focused text field; does nothing when a button is focused.
    pub fn edit(&mut self, f: impl FnOnce(&mut TextInput)) {
        match self.focus {
            Focus::Date => f(&mut self.date),
            Focus::Name => f(&mut self.name),
            Focus::Delete | Focus::Save | Focus::Cancel => {}
        }
    }

    /// Tab (`step` = 1) or Shift-Tab (`step` = -1): wraps around.
    pub fn tab(&mut self, step: isize) {
        let order = self.order();
        let at = order.iter().position(|f| *f == self.focus).unwrap_or(0) as isize;
        let next = (at + step).rem_euclid(order.len() as isize) as usize;
        self.focus = order[next];
    }

    /// Row up: Name -> Date, buttons -> Name.
    pub fn up(&mut self) {
        self.focus = match self.focus {
            Focus::Date | Focus::Name => Focus::Date,
            _ => Focus::Name,
        };
    }

    /// Row down: Date -> Name, Name -> Save; the button row is the last.
    pub fn down(&mut self) {
        self.focus = match self.focus {
            Focus::Date => Focus::Name,
            Focus::Name => Focus::Save,
            button => button,
        };
    }

    /// Cursor left in a field, previous button on the button row (no wrap).
    pub fn left(&mut self) {
        if self.on_button() { self.step_button(-1) } else { self.edit(TextInput::left) }
    }

    /// Cursor right in a field, next button on the button row (no wrap).
    pub fn right(&mut self) {
        if self.on_button() { self.step_button(1) } else { self.edit(TextInput::right) }
    }

    fn step_button(&mut self, step: isize) {
        let buttons = self.buttons();
        let at = buttons.iter().position(|f| *f == self.focus).unwrap_or(0) as isize;
        self.focus = buttons[(at + step).clamp(0, buttons.len() as isize - 1) as usize];
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// A click on a text field.
    Focus(Focus),
    Next,
    Prev,
    Up,
    Down,
    Char(char),
    Paste(String),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    /// Enter: leaves a field for the next row, or presses the focused button.
    Enter,
    /// The `[ Save ]` button.
    Save,
    /// The `[ Delete ]` button: only does anything when editing an existing
    /// day off.
    Remove,
    Cancel,
}
