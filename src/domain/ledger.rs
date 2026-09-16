use super::{Entry, EntryId, EntryState, Intent, IssueKey, RemoteWorklog, StartTime};
use chrono::{NaiveDate, Timelike};

/// Everything the user owns locally: entries + watchlist (keys only —
/// summaries always come fresh from Jira, never from local state).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ledger {
    entries: Vec<Entry>,
    watchlist: Vec<IssueKey>,
}

impl Ledger {
    pub fn new(entries: Vec<Entry>, watchlist: Vec<IssueKey>) -> Self {
        Self { entries, watchlist }
    }

    // ---- entries ------------------------------------------------------

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    #[cfg(test)]
    pub fn entry(&self, id: &EntryId) -> Option<&Entry> {
        self.entries.iter().find(|e| &e.id == id)
    }

    pub fn entries_on(&self, date: NaiveDate) -> Vec<&Entry> {
        self.entries.iter().filter(|e| e.date == date).collect()
    }

    pub fn staged_on(&self, date: NaiveDate) -> Vec<&Entry> {
        self.entries.iter().filter(|e| e.date == date && e.needs_push()).collect()
    }

    pub fn staged_in(&self, from: NaiveDate, to: NaiveDate) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.date >= from && e.date <= to && e.needs_push())
            .collect()
    }

    pub fn staged_count(&self) -> usize {
        self.entries.iter().filter(|e| e.needs_push()).count()
    }

    /// Quick stage: default duration and start, no description.
    pub fn quick_stage(&mut self, id: EntryId, key: IssueKey, date: NaiveDate, start: StartTime, seconds: u64) -> &Entry {
        self.entries.push(Entry {
            id,
            issue_key: key,
            date,
            start,
            seconds,
            title: String::new(),
            detail: String::new(),
            state: EntryState::Staged,
        });
        self.entries.last().unwrap()
    }

    pub fn add(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    pub fn update<F: FnOnce(&mut Entry)>(&mut self, id: &EntryId, f: F) -> bool {
        match self.entries.iter_mut().find(|e| &e.id == id) {
            Some(e) => {
                f(e);
                true
            }
            None => false,
        }
    }

    pub fn remove(&mut self, id: &EntryId) -> Option<Entry> {
        let i = self.entries.iter().position(|e| &e.id == id)?;
        Some(self.entries.remove(i))
    }

    /// Edit fields of an entry. A pushed entry becomes `Modified` so the
    /// change is sent on the next push; other states keep their intent.
    pub fn edit<F: FnOnce(&mut Entry)>(&mut self, id: &EntryId, f: F) -> bool {
        self.update(id, |e| {
            f(e);
            if let EntryState::Pushed { worklog_id } = &e.state {
                e.state = EntryState::Modified { worklog_id: worklog_id.clone() };
            }
        })
    }

    /// Backspace on a row: local-only rows disappear, Jira rows toggle a
    /// pending delete. Returns true if the entry was removed outright.
    pub fn toggle_delete(&mut self, id: &EntryId) -> bool {
        let Some(e) = self.entries.iter_mut().find(|e| &e.id == id) else { return false };
        let next = match &e.state {
            EntryState::Staged | EntryState::Failed { intent: Intent::Create, .. } => None,
            EntryState::Pushed { worklog_id } | EntryState::Modified { worklog_id } => Some(EntryState::Deleted { worklog_id: worklog_id.clone() }),
            EntryState::Failed { intent: Intent::Update { worklog_id }, .. } => Some(EntryState::Deleted { worklog_id: worklog_id.clone() }),
            EntryState::Deleted { worklog_id } | EntryState::Failed { intent: Intent::Delete { worklog_id }, .. } => {
                Some(EntryState::Pushed { worklog_id: worklog_id.clone() })
            }
        };
        match next {
            Some(s) => {
                e.state = s;
                false
            }
            None => {
                self.remove(id);
                true
            }
        }
    }

    /// Bring a Jira-only worklog under local control so it can be edited or
    /// deleted. Returns the existing id if already adopted.
    pub fn adopt_remote(&mut self, id: EntryId, w: &RemoteWorklog) -> EntryId {
        if let Some(e) = self.entries.iter().find(|e| e.worklog_id() == Some(w.id.as_str())) {
            return e.id.clone();
        }
        let local = w.started.with_timezone(&chrono::Local);
        self.entries.push(Entry {
            id: id.clone(),
            issue_key: w.issue_key.clone(),
            date: w.local_date(),
            start: StartTime::from_hm(local.hour() as u16, local.minute() as u16).unwrap_or(StartTime::NINE),
            seconds: w.seconds,
            title: w.comment.clone(),
            detail: String::new(),
            state: EntryState::Pushed { worklog_id: w.id.clone() },
        });
        id
    }

    /// Jira is the source of truth for `Pushed` rows: after a sync, copy the
    /// remote values onto them and drop rows Jira no longer has. Rows with a
    /// pending intent (modified / deleted / failed) are left alone.
    /// `covered` = issue keys the sync actually fetched worklogs for; pushed
    /// rows on other issues are kept as-is.
    pub fn reconcile(&mut self, remote: &[RemoteWorklog], covered: &[IssueKey]) {
        self.entries.retain_mut(|e| {
            if !e.is_pushed() || !covered.contains(&e.issue_key) {
                return true;
            }
            let Some(id) = e.worklog_id() else { return true };
            match remote.iter().find(|w| w.id == id) {
                Some(w) => {
                    let local = w.started.with_timezone(&chrono::Local);
                    e.date = w.local_date();
                    e.start = StartTime::from_hm(local.hour() as u16, local.minute() as u16).unwrap_or(e.start);
                    e.seconds = w.seconds;
                    e.title = w.comment.clone();
                    e.detail.clear();
                    true
                }
                None => false,
            }
        });
    }

    pub fn mark_pushed(&mut self, id: &EntryId, worklog_id: String) {
        self.update(id, |e| e.state = EntryState::Pushed { worklog_id });
    }

    pub fn mark_failed(&mut self, id: &EntryId, error: String) {
        self.update(id, |e| {
            let intent = e.intent().unwrap_or(Intent::Create);
            e.state = EntryState::Failed { error, intent };
        });
    }

    // ---- watchlist ----------------------------------------------------

    pub fn watchlist(&self) -> &[IssueKey] {
        &self.watchlist
    }

    pub fn is_watched(&self, key: &IssueKey) -> bool {
        self.watchlist.contains(key)
    }

    /// Returns false if already present.
    pub fn watch(&mut self, key: &IssueKey) -> bool {
        if self.is_watched(key) {
            return false;
        }
        self.watchlist.push(key.clone());
        true
    }

    pub fn unwatch(&mut self, key: &IssueKey) -> bool {
        let before = self.watchlist.len();
        self.watchlist.retain(|w| w != key);
        self.watchlist.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(s: &str) -> IssueKey {
        IssueKey::parse(s).unwrap()
    }
    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn stage_edit_remove() {
        let mut l = Ledger::default();
        l.quick_stage(EntryId::new("1".into()), key("A-1"), d("2026-09-16"), StartTime::NINE, 3600);
        assert_eq!(l.staged_on(d("2026-09-16")).len(), 1);
        assert!(l.update(&EntryId::new("1".into()), |e| e.seconds = 1800));
        assert_eq!(l.entry(&EntryId::new("1".into())).unwrap().seconds, 1800);
        l.mark_pushed(&EntryId::new("1".into()), "99".into());
        assert_eq!(l.staged_on(d("2026-09-16")).len(), 0);
        assert!(l.remove(&EntryId::new("1".into())).is_some());
        assert!(l.entries().is_empty());
    }

    #[test]
    fn reconcile_takes_remote_values_and_drops_missing() {
        use chrono::{FixedOffset, TimeZone};
        let mut l = Ledger::default();
        for (id, wid) in [("1", "w1"), ("2", "w2"), ("3", "w3")] {
            l.quick_stage(EntryId::new(id.into()), key("A-1"), d("2026-09-16"), StartTime::NINE, 3600);
            l.mark_pushed(&EntryId::new(id.into()), wid.into());
        }
        // entry 3 has a pending edit → must be left alone
        l.edit(&EntryId::new("3".into()), |e| e.seconds = 60);
        let started = FixedOffset::east_opt(7 * 3600).unwrap().with_ymd_and_hms(2026, 9, 16, 13, 30, 0).unwrap();
        let remote = vec![RemoteWorklog { id: "w1".into(), issue_key: key("A-1"), started, seconds: 5400, comment: "changed in jira".into() }];
        l.reconcile(&remote, &[key("A-1")]);
        let e1 = l.entry(&EntryId::new("1".into())).unwrap();
        assert_eq!((e1.seconds, e1.title.as_str()), (5400, "changed in jira"));
        assert_eq!(e1.start, StartTime::from_hm(started.with_timezone(&chrono::Local).hour() as u16, 30).unwrap());
        assert!(l.entry(&EntryId::new("2".into())).is_none(), "deleted in jira → dropped");
        assert_eq!(l.entry(&EntryId::new("3".into())).unwrap().seconds, 60, "pending edit untouched");
    }

    #[test]
    fn pushed_edit_and_delete_lifecycle() {
        let id = EntryId::new("1".into());
        let mut l = Ledger::default();
        l.quick_stage(id.clone(), key("A-1"), d("2026-09-16"), StartTime::NINE, 3600);
        l.mark_pushed(&id, "77".into());
        l.edit(&id, |e| e.seconds = 1800);
        assert_eq!(l.entry(&id).unwrap().intent(), Some(Intent::Update { worklog_id: "77".into() }));
        l.mark_failed(&id, "boom".into());
        assert_eq!(l.entry(&id).unwrap().worklog_id(), Some("77"));
        assert!(l.entry(&id).unwrap().is_modified());
        assert!(!l.toggle_delete(&id));
        assert!(l.entry(&id).unwrap().is_deleted());
        assert!(!l.toggle_delete(&id));
        assert!(l.entry(&id).unwrap().is_pushed());
        l.quick_stage(EntryId::new("2".into()), key("A-1"), d("2026-09-16"), StartTime::NINE, 3600);
        assert!(l.toggle_delete(&EntryId::new("2".into())));
        assert_eq!(l.entries().len(), 1);
    }

    #[test]
    fn watchlist_dedup() {
        let mut l = Ledger::default();
        assert!(l.watch(&key("OPS-7")));
        assert!(!l.watch(&key("OPS-7")));
        assert!(l.is_watched(&key("ops-7")));
        assert!(l.unwatch(&key("OPS-7")));
        assert!(!l.unwatch(&key("OPS-7")));
    }
}
