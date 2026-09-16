use crate::domain::EntryId;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Unique enough for one user's machine: unix-nanos + process counter.
pub fn new_entry_id() -> EntryId {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    EntryId::new(format!("{nanos:x}-{n:x}"))
}
