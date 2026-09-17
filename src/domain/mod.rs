//! Pure business rules. No IO, no terminal, no HTTP.

pub mod calendar;
pub mod duration;
pub mod entry;
pub mod issue;
pub mod ledger;
pub mod site;
pub mod start_time;
pub mod week;
pub mod worklog;

pub use calendar::WorkCalendar;
pub use entry::{Entry, EntryId, EntryState, Intent};
pub use issue::{Issue, IssueKey, StatusCategory, due_label, updated_label};
pub use ledger::Ledger;
pub use site::SiteUrl;
pub use start_time::StartTime;
pub use week::{DayStatus, DaySummary, Week};
pub use worklog::RemoteWorklog;
#[cfg(test)]
mod entry_tests;
#[cfg(test)]
mod ledger_tests;
#[cfg(test)]
mod week_tests;
#[cfg(test)]
mod calendar_tests;
#[cfg(test)]
mod entry_description_tests;
#[cfg(test)]
mod ledger_reconcile_tests;
