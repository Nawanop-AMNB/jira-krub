use super::{Entry, RemoteWorklog};
use chrono::{Datelike, Days, NaiveDate, Weekday};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Week {
    monday: NaiveDate,
}

impl Week {
    pub fn containing(d: NaiveDate) -> Self {
        Self { monday: d - Days::new(d.weekday().num_days_from_monday() as u64) }
    }
    pub fn monday(&self) -> NaiveDate {
        self.monday
    }
    pub fn sunday(&self) -> NaiveDate {
        self.monday + Days::new(6)
    }
    pub fn days(&self) -> [NaiveDate; 7] {
        std::array::from_fn(|i| self.monday + Days::new(i as u64))
    }
    pub fn prev(&self) -> Self {
        Self { monday: self.monday - Days::new(7) }
    }
    pub fn next(&self) -> Self {
        Self { monday: self.monday + Days::new(7) }
    }
    pub fn contains(&self, d: NaiveDate) -> bool {
        d >= self.monday && d <= self.sunday()
    }
    pub fn index_of(&self, d: NaiveDate) -> Option<usize> {
        self.contains(d).then(|| (d - self.monday).num_days() as usize)
    }
}

pub fn is_workday(d: NaiveDate) -> bool {
    !matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayStatus {
    /// ≥ target
    Full,
    /// some hours, under target (past or today)
    Short,
    /// zero hours on a past workday
    Empty,
    /// today with zero hours
    TodayEmpty,
    Future,
    Weekend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaySummary {
    pub date: NaiveDate,
    pub pushed_seconds: u64,
    pub staged_seconds: u64,
    pub staged_count: usize,
    pub status: DayStatus,
}

impl DaySummary {
    pub fn total(&self) -> u64 {
        self.pushed_seconds + self.staged_seconds
    }
    pub fn remaining(&self, target: u64) -> u64 {
        target.saturating_sub(self.total())
    }

    /// Pushed seconds = remote worklogs ∪ local pushed entries whose worklog
    /// id is not (yet) in the remote set. Staged = local entries needing push.
    pub fn compute(
        date: NaiveDate,
        today: NaiveDate,
        target: u64,
        entries: &[Entry],
        remote: &[RemoteWorklog],
    ) -> Self {
        let remote_today: Vec<&RemoteWorklog> = remote.iter().filter(|w| w.local_date() == date).collect();
        let remote_secs: HashMap<&str, u64> = remote_today.iter().map(|w| (w.id.as_str(), w.seconds)).collect();
        let mut pushed: u64 = remote_today.iter().map(|w| w.seconds).sum();
        let mut staged = 0u64;
        let mut staged_count = 0usize;
        for e in entries.iter().filter(|e| e.date == date) {
            let known_remote = e.worklog_id().and_then(|id| remote_secs.get(id).copied());
            if e.is_pushed() {
                // Counted via the remote list when synced; otherwise pushed just now.
                if known_remote.is_none() {
                    pushed += e.seconds;
                }
                continue;
            }
            // Pending change: what Jira has no longer counts, the intent does.
            if let Some(old) = known_remote {
                pushed -= old;
            }
            staged_count += 1;
            if !e.is_deleted() {
                staged += e.seconds;
            }
        }
        let total = pushed + staged;
        let status = if !is_workday(date) {
            DayStatus::Weekend
        } else if date > today {
            DayStatus::Future
        } else if total >= target {
            DayStatus::Full
        } else if total > 0 {
            DayStatus::Short
        } else if date == today {
            DayStatus::TodayEmpty
        } else {
            DayStatus::Empty
        };
        Self { date, pushed_seconds: pushed, staged_seconds: staged, staged_count, status }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EntryId, EntryState, IssueKey, StartTime};
    use chrono::{FixedOffset, TimeZone};

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }
    fn entry(id: &str, date: &str, secs: u64, state: EntryState) -> Entry {
        Entry {
            id: EntryId::new(id.into()),
            issue_key: IssueKey::parse("A-1").unwrap(),
            date: d(date),
            start: StartTime::NINE,
            seconds: secs,
            title: "t".into(),
            detail: String::new(),
            state,
        }
    }

    #[test]
    fn week_math() {
        let w = Week::containing(d("2026-09-16")); // Wed
        assert_eq!(w.monday(), d("2026-09-14"));
        assert_eq!(w.sunday(), d("2026-09-20"));
        assert_eq!(w.index_of(d("2026-09-16")), Some(2));
        assert_eq!(w.next().monday(), d("2026-09-21"));
        assert!(!w.contains(d("2026-09-21")));
    }

    #[test]
    fn summary_merges_and_classifies() {
        let today = d("2026-09-16");
        let target = 8 * 3600;
        let remote = vec![RemoteWorklog {
            id: "r1".into(),
            issue_key: IssueKey::parse("A-1").unwrap(),
            started: FixedOffset::east_opt(7 * 3600).unwrap().with_ymd_and_hms(2026, 9, 16, 9, 0, 0).unwrap(),
            seconds: 3600,
            comment: String::new(),
        }];
        let entries = vec![
            entry("1", "2026-09-16", 3600, EntryState::Pushed { worklog_id: "r1".into() }), // already remote → not double counted
            entry("2", "2026-09-16", 1800, EntryState::Pushed { worklog_id: "r2".into() }), // pushed, not yet synced
            entry("3", "2026-09-16", 7200, EntryState::Staged),
            entry("4", "2026-09-16", 900, EntryState::Failed { error: "x".into(), intent: crate::domain::Intent::Create }),
        ];
        let s = DaySummary::compute(today, today, target, &entries, &remote);
        assert_eq!(s.pushed_seconds, 5400);
        assert_eq!(s.staged_seconds, 8100);
        assert_eq!(s.staged_count, 2);
        assert_eq!(s.status, DayStatus::Short);
        assert_eq!(s.remaining(target), 8 * 3600 - 13500);

        let entries2 = vec![entry("5", "2026-09-16", 1800, EntryState::Modified { worklog_id: "r1".into() })];
        let s2 = DaySummary::compute(today, today, target, &entries2, &remote);
        assert_eq!((s2.pushed_seconds, s2.staged_seconds, s2.staged_count), (0, 1800, 1));
        let entries3 = vec![entry("6", "2026-09-16", 3600, EntryState::Deleted { worklog_id: "r1".into() })];
        let s3 = DaySummary::compute(today, today, target, &entries3, &remote);
        assert_eq!((s3.pushed_seconds, s3.staged_seconds, s3.staged_count), (0, 0, 1));

        assert_eq!(DaySummary::compute(d("2026-09-15"), today, target, &[], &[]).status, DayStatus::Empty);
        assert_eq!(DaySummary::compute(d("2026-09-17"), today, target, &[], &[]).status, DayStatus::Future);
        assert_eq!(DaySummary::compute(d("2026-09-19"), today, target, &[], &[]).status, DayStatus::Weekend);
        assert_eq!(DaySummary::compute(today, today, target, &[], &[]).status, DayStatus::TodayEmpty);
    }
}
