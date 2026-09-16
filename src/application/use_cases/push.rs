use crate::application::ports::{JiraGateway, NewWorklog};
use crate::domain::{Entry, EntryId, Intent, IssueKey, RemoteWorklog};
use anyhow::Result;
use chrono::{Local, TimeZone};

#[derive(Debug, Clone)]
pub enum Op {
    Create(NewWorklog),
    Update { worklog_id: String, request: NewWorklog },
    Delete { issue_key: IssueKey, worklog_id: String },
}

#[derive(Debug, Clone)]
pub struct PushItem {
    pub id: EntryId,
    pub op: Op,
}

#[derive(Debug, Clone)]
pub enum Outcome {
    Saved(RemoteWorklog),
    Deleted { worklog_id: String },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub create: usize,
    pub update: usize,
    pub delete: usize,
    pub seconds: u64,
}

fn request_for(e: &Entry) -> Option<NewWorklog> {
    let naive = e.date.and_hms_opt(e.start.hour() as u32, e.start.minute() as u32, 0)?;
    let started = Local.from_local_datetime(&naive).single()?;
    Some(NewWorklog {
        issue_key: e.issue_key.clone(),
        started,
        seconds: e.seconds,
        comment_paragraphs: e.comment_paragraphs(),
    })
}

/// One item per entry that still needs Jira. Untitled entries are skipped
/// and reported when `allow_untitled` is false.
pub fn plan(entries: &[&Entry], allow_untitled: bool) -> (Vec<PushItem>, Vec<EntryId>) {
    let mut items = Vec::new();
    let mut untitled = Vec::new();
    for e in entries {
        let Some(intent) = e.intent() else { continue };
        if !e.has_title() && !allow_untitled && !matches!(intent, Intent::Delete { .. }) {
            untitled.push(e.id.clone());
            continue;
        }
        let op = match intent {
            Intent::Create => match request_for(e) {
                Some(r) => Op::Create(r),
                None => continue,
            },
            Intent::Update { worklog_id } => match request_for(e) {
                Some(r) => Op::Update { worklog_id, request: r },
                None => continue,
            },
            Intent::Delete { worklog_id } => Op::Delete { issue_key: e.issue_key.clone(), worklog_id },
        };
        items.push(PushItem { id: e.id.clone(), op });
    }
    (items, untitled)
}

pub fn counts(items: &[PushItem]) -> Counts {
    let mut c = Counts::default();
    for i in items {
        match &i.op {
            Op::Create(r) => {
                c.create += 1;
                c.seconds += r.seconds;
            }
            Op::Update { request, .. } => {
                c.update += 1;
                c.seconds += request.seconds;
            }
            Op::Delete { .. } => c.delete += 1,
        }
    }
    c
}

pub fn push_one(gateway: &dyn JiraGateway, item: &PushItem) -> Result<Outcome> {
    match &item.op {
        Op::Create(r) => gateway.add_worklog(r).map(Outcome::Saved),
        Op::Update { worklog_id, request } => gateway.update_worklog(worklog_id, request).map(Outcome::Saved),
        Op::Delete { issue_key, worklog_id } => {
            gateway.delete_worklog(issue_key, worklog_id)?;
            Ok(Outcome::Deleted { worklog_id: worklog_id.clone() })
        }
    }
}
