use crate::application::Credentials;
use crate::application::ports::GatewayErrorKind;
use crate::application::use_cases::{push, sync::SyncOutput, test_connection};
use crate::domain::{EntryId, Issue};

/// Results arriving from background work.
pub enum Msg {
    ConnectionTested { creds: Credentials, outcome: test_connection::Outcome },
    /// The assigned-to-me list has landed; the rest of the sync is still in
    /// flight (see `sync::SyncProgress::Mine`).
    SyncMine { account_id: String, display_name: String, mine: Vec<Issue> },
    Synced(Result<SyncOutput, SyncError>),
    SearchDone { req_id: u64, query: String, result: Result<Vec<Issue>, String> },
    PushProgress { id: EntryId, result: Result<push::Outcome, String> },
    PushDone { ok: usize, failed: usize },
}

pub struct SyncError {
    pub kind: GatewayErrorKind,
    pub message: String,
}
