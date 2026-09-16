use crate::application::ports::JiraGateway;
use crate::domain::{Issue, IssueKey, RemoteWorklog};
use anyhow::Result;
use chrono::NaiveDate;
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct SyncInput {
    pub jql: String,
    pub watchlist: Vec<IssueKey>,
    /// Look back this far for worklog-history issues.
    pub history_from: NaiveDate,
}

#[derive(Debug, Clone, Default)]
pub struct SyncOutput {
    pub account_id: String,
    pub display_name: String,
    /// Assigned, un-done (config JQL).
    pub mine: Vec<Issue>,
    /// Watchlist issues, freshly fetched (missing ones are skipped).
    pub watched: Vec<Issue>,
    /// Issues with my worklogs in range that are neither mine nor watched.
    pub history: Vec<Issue>,
    pub worklogs: Vec<RemoteWorklog>,
}

pub fn run(gateway: &dyn JiraGateway, input: &SyncInput) -> Result<SyncOutput> {
    let me = gateway.myself()?;
    let mine = gateway.search_issues(&input.jql, 200)?;

    let mut watched = Vec::new();
    for key in &input.watchlist {
        if let Ok(issue) = gateway.get_issue(key) {
            watched.push(issue);
        }
    }

    let history_jql = format!(
        "worklogAuthor = currentUser() AND worklogDate >= {} ORDER BY updated DESC",
        input.history_from.format("%Y-%m-%d")
    );
    let known: BTreeSet<&IssueKey> = mine.iter().chain(watched.iter()).map(|i| &i.key).collect();
    let history: Vec<Issue> = gateway
        .search_issues(&history_jql, 200)?
        .into_iter()
        .filter(|i| !known.contains(&i.key))
        .collect();

    let mut worklogs = Vec::new();
    for issue in mine.iter().chain(watched.iter()).chain(history.iter()) {
        worklogs.extend(gateway.my_worklogs(&issue.key, &me.account_id)?);
    }

    Ok(SyncOutput {
        account_id: me.account_id,
        display_name: me.display_name,
        mine,
        watched,
        history,
        worklogs,
    })
}
