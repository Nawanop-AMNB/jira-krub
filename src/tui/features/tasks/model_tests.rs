//! Grouping rules for the My tasks tab (R17).

use super::model::group_issues;
use crate::application::test_support::{issue, key};
use crate::domain::{Issue, StatusCategory};

fn task(k: &str, status: &str, category: StatusCategory) -> Issue {
    let mut i = issue(k);
    i.status = status.to_string();
    i.status_category = category;
    i
}

#[test]
fn groups_order_by_category_then_name_and_drop_done() {
    use StatusCategory::*;
    let issues = vec![
        task("KAN-1", "To Do", New),
        task("KAN-2", "In Progress", Indeterminate),
        task("KAN-3", "Done", Done),
        task("KAN-4", "In Review", Indeterminate),
        task("KAN-5", "Backlog", New),
    ];
    let names: Vec<String> = group_issues(&issues).into_iter().map(|g| g.status).collect();
    assert_eq!(names, vec!["In Progress", "In Review", "Backlog", "To Do"], "indeterminate first, then alphabetical inside each category");
}

#[test]
fn group_keeps_input_order_inside() {
    use StatusCategory::*;
    // `KAN-9` first even though `KAN-2` sorts before it: input order wins.
    let issues = vec![task("KAN-9", "In Progress", Indeterminate), task("KAN-2", "In Progress", Indeterminate)];
    let groups = group_issues(&issues);
    assert_eq!(groups.len(), 1);
    let keys: Vec<_> = groups[0].issues.iter().map(|i| i.key.clone()).collect();
    assert_eq!(keys, vec![key("KAN-9"), key("KAN-2")], "the JQL already sorted by updated DESC");
}
