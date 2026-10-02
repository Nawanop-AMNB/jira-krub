use crate::domain::{Head, Issue, IssueKey, Nested, StatusCategory, nest};
use crate::tui::app::{App, Screen};
use crate::tui::widgets::TextInput;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub status: String,
    pub category: StatusCategory,
    pub issues: Vec<Issue>,
}

/// Sort key for a status category: what is being worked on comes first.
fn rank(category: StatusCategory) -> u8 {
    match category {
        StatusCategory::Indeterminate => 0,
        StatusCategory::New => 1,
        StatusCategory::Done => 2,
    }
}

/// Drops `Done` issues, groups the rest by `status` (Jira's status *name*,
/// exact string), sorted by status **category** (`Indeterminate` before
/// `New`) then alphabetically by status name within a category. Issues
/// keep their input order inside a group (already `updated` DESC from the
/// JQL — do not re-sort them).
///
/// Each assigned issue's own open (non-Done) sub-tasks are then appended to
/// its group — not the sub-task's own status group, since it rides along
/// with the card it was fetched for (R2/R4). A sub-task already present as
/// its own assigned issue is not duplicated.
pub fn group_issues(issues: &[Issue]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for issue in issues.iter().filter(|i| i.status_category != StatusCategory::Done) {
        match groups.iter_mut().find(|g| g.status == issue.status) {
            Some(g) => g.issues.push(issue.clone()),
            None => groups.push(Group { status: issue.status.clone(), category: issue.status_category, issues: vec![issue.clone()] }),
        }
    }
    let mut seen: HashSet<IssueKey> = groups.iter().flat_map(|g| g.issues.iter().map(|i| i.key.clone())).collect();
    for g in &mut groups {
        let mut children = Vec::new();
        for parent in &g.issues {
            for child in &parent.subtasks {
                if child.status_category != StatusCategory::Done && seen.insert(child.key.clone()) {
                    children.push(child.clone());
                }
            }
        }
        g.issues.extend(children);
    }
    groups.sort_by(|a, b| rank(a.category).cmp(&rank(b.category)).then_with(|| a.status.cmp(&b.status)));
    groups
}

/// The groups actually on screen: assigned issues after the filter box, with
/// groups left empty by it dropped.
pub fn visible_groups(app: &App, m: &Model) -> Vec<Group> {
    let needle = m.filter.text().trim().to_string();
    let mut groups = group_issues(&app.remote.mine);
    for g in &mut groups {
        g.issues.retain(|i| i.matches(&needle));
    }
    groups.retain(|g| !g.issues.is_empty());
    groups
}

/// Each status group's issues nested into parent/child/context display rows
/// (R2/R4): a sub-task goes under its parent's row when the parent is also
/// in that status group, otherwise under a `Context` head. Nesting never
/// crosses a status group, same as it never crosses a day-view section.
pub fn visible_nested(app: &App, m: &Model) -> Vec<(Group, Vec<Nested>)> {
    visible_groups(app, m).into_iter().map(|g| { let nested = nest(&g.issues); (g, nested) }).collect()
}

/// Selectable issues in render order: skip `Context` heads, which are not
/// selectable — what `Model::selected` indexes.
pub fn visible_issues(app: &App, m: &Model) -> Vec<Issue> {
    visible_nested(app, m)
        .into_iter()
        .flat_map(|(_, nested)| nested.into_iter().flat_map(|n| {
            let mut v = Vec::new();
            if let Head::Issue(i) = n.head {
                v.push(i);
            }
            v.extend(n.children);
            v
        }))
        .collect()
}

/// Assigned issues that are not done, before filtering — the box title's `N`.
pub fn open_count(app: &App) -> usize {
    app.remote.mine.iter().filter(|i| i.status_category != StatusCategory::Done).count()
}

#[derive(Debug)]
pub struct Model {
    pub filter: TextInput,
    pub filter_focused: bool,
    /// Index into the flattened, filtered, header-free list of issues.
    pub selected: usize,
}

impl Model {
    pub fn new() -> Self {
        Self { filter: TextInput::default(), filter_focused: false, selected: 0 }
    }
}

/// The tasks model, when that is the screen we are on.
pub fn model(app: &mut App) -> Option<&mut Model> {
    match &mut app.screen {
        Screen::Tasks(m) => Some(m),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    FocusFilter,
    FilterChar(char),
    FilterBackspace,
    FilterDelete,
    /// Cursor moves inside the filter box: -1 / +1 / home (i32::MIN) / end (i32::MAX).
    FilterCursor(i32),
    FilterClear,
    Up,
    Down,
    Select(usize),
    /// Double-click: put the cursor on the row, then open it.
    SelectAndOpen(usize),
    Open,
}
