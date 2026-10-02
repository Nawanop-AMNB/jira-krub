use crate::domain::{EntryId, Issue, IssueKey};
use crate::tui::widgets::TextInput;
use chrono::NaiveDate;
use std::collections::{HashMap, HashSet};
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
    /// Jira-section groups expanded past the `… +N more` cap, keyed by group
    /// (parent) key. Reset whenever the search query text changes.
    pub expanded: HashSet<IssueKey>,
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
    /// Pending-row order captured when an Enter walk starts. While set, the
    /// prepare pane keeps this order so the row under the cursor never moves
    /// mid-walk; cleared (and the list re-sorted) when the walk ends.
    pub frozen: Option<Vec<String>>,
    /// Entry being walked, to re-find it after the re-sort.
    pub walk_id: Option<EntryId>,
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
            frozen: None,
            walk_id: None,
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
    SearchChar(char),
    SearchBackspace,
    SearchDelete,
    SearchClear,
    /// Cursor moves inside the search box: -1 / +1 / home (i32::MIN) / end (i32::MAX).
    SearchCursor(i32),
    Paste(String),
    Up,
    Down,
    SelectTicket(usize),
    SelectPrepare(usize),
    QuickStage,
    StageKey(IssueKey),
    /// Lifts the `… +N more` cap for the jira-section group keyed by this
    /// parent key, showing every child.
    ExpandGroup(IssueKey),
    OpenForm,
    ToggleWatch,
    EditCell(Cell),
    EditCellOf(usize, Cell),
    CellChar(char),
    /// Ctrl+Enter / Ctrl+J while editing the description cell.
    CellNewline,
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
