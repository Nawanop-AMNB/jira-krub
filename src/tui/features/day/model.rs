use crate::domain::{EntryId, Issue, IssueKey};
use crate::tui::widgets::TextInput;
use chrono::NaiveDate;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Tickets,
    Prepare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Start,
    Duration,
    Title,
}

impl Cell {
    pub fn next(self) -> Option<Self> {
        match self {
            Cell::Start => Some(Cell::Duration),
            Cell::Duration => Some(Cell::Title),
            Cell::Title => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellEdit {
    pub id: EntryId,
    pub cell: Cell,
    pub input: TextInput,
    pub error: Option<String>,
    /// Untouched since opening: the first typed char replaces the whole value.
    pub pristine: bool,
}

#[derive(Debug, Default)]
pub struct SearchState {
    pub dirty_since: Option<Instant>,
    pub sent: Option<String>,
    pub req_id: u64,
    pub results: Vec<Issue>,
    pub loading: bool,
    pub cache: HashMap<String, Vec<Issue>>,
}

#[derive(Debug)]
pub struct Model {
    pub date: NaiveDate,
    pub pane: Pane,
    pub search: TextInput,
    pub search_focused: bool,
    pub s: SearchState,
    pub ticket_sel: usize,
    pub prepare_sel: usize,
    pub edit: Option<CellEdit>,
}

impl Model {
    pub fn new(date: NaiveDate) -> Self {
        Self {
            date,
            pane: Pane::Tickets,
            search: TextInput::default(),
            search_focused: false,
            s: SearchState::default(),
            ticket_sel: 0,
            prepare_sel: 0,
            edit: None,
        }
    }
    pub fn query(&self) -> String {
        self.search.text().trim().to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    FocusTickets,
    FocusPrepare,
    FocusSearch,
    TogglePane,
    SearchChar(char),
    SearchBackspace,
    SearchClear,
    Paste(String),
    Up,
    Down,
    SelectTicket(usize),
    SelectPrepare(usize),
    QuickStage,
    StageKey(IssueKey),
    OpenForm,
    ToggleWatch,
    EditCell(Cell),
    EditCellOf(usize, Cell),
    CellChar(char),
    CellBackspace,
    CellDelete,
    CellLeft,
    CellRight,
    CellHome,
    CellEnd,
    CellStep(i32),
    CellNext,
    CellCancel,
    CellCursor(u16),
    Remove,
    PrevDay,
    NextDay,
    Back,
    PushDay,
}
