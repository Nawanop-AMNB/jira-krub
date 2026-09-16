//! Batch push of pending entries (create / update / delete) with per-entry outcome.

use crate::application::use_cases::push::{Outcome, counts, plan, push_one};
use crate::domain::{EntryId, duration};
use crate::tui::action::{Action, PushScope};
use crate::tui::app::{App, PushProgress};
use crate::tui::msg::Msg;

fn pending_for(app: &App, scope: PushScope) -> Vec<&crate::domain::Entry> {
    match scope {
        PushScope::Day(d) => app.ledger.staged_on(d),
        PushScope::Week(w) => app.ledger.staged_in(w.monday(), w.sunday()),
    }
}

pub fn request(app: &mut App, scope: PushScope) {
    if app.gateway.is_none() {
        app.set_error("not connected — open settings with ,");
        return;
    }
    if app.push.is_some() {
        app.set_error("push already running");
        return;
    }
    let entries = pending_for(app, scope);
    // Title is optional: untitled entries are pushed without a comment.
    let (items, _) = plan(&entries, true);
    if items.is_empty() {
        let label = app.push_scope_label(scope);
        app.set_status(format!("nothing pending for {label}"));
        return;
    }
    let c = counts(&items);
    let mut parts = Vec::new();
    if c.create > 0 {
        parts.push(format!("{} new", c.create));
    }
    if c.update > 0 {
        parts.push(format!("{} edited", c.update));
    }
    if c.delete > 0 {
        parts.push(format!("{} delete", c.delete));
    }
    let label = app.push_scope_label(scope);
    app.confirm(
        format!("push {} ({}) for {label} to Jira? [y/n]", parts.join(" · "), duration::format(c.seconds)),
        Action::PushConfirmed(scope),
    );
}

pub fn confirmed(app: &mut App, scope: PushScope) {
    let Some(gateway) = app.gateway.clone() else { return };
    let entries = pending_for(app, scope);
    let (items, _) = plan(&entries, true);
    if items.is_empty() {
        return;
    }
    app.push = Some(PushProgress { total: items.len(), done: 0 });
    app.set_busy(format!("pushing 0/{}…", items.len()));
    app.worker.spawn_streaming(move |tx| {
        let (mut ok, mut failed) = (0, 0);
        for item in &items {
            let result = push_one(gateway.as_ref(), item).map_err(|e| e.to_string());
            if result.is_ok() {
                ok += 1;
            } else {
                failed += 1;
            }
            if tx.send(Msg::PushProgress { id: item.id.clone(), result }).is_err() {
                return;
            }
        }
        let _ = tx.send(Msg::PushDone { ok, failed });
    });
}

pub fn on_progress(app: &mut App, id: EntryId, result: Result<Outcome, String>) {
    match result {
        Ok(Outcome::Saved(w)) => {
            app.ledger.mark_pushed(&id, w.id.clone());
            app.remote.worklogs.retain(|r| r.id != w.id);
            app.remote.worklogs.push(w);
        }
        Ok(Outcome::Deleted { worklog_id }) => {
            app.ledger.remove(&id);
            app.remote.worklogs.retain(|r| r.id != worklog_id);
        }
        Err(e) => app.ledger.mark_failed(&id, e),
    }
    app.save_ledger();
    if let Some(p) = &mut app.push {
        p.done += 1;
        let (done, total) = (p.done, p.total);
        app.set_busy(format!("pushing {done}/{total}…"));
    }
}

pub fn on_done(app: &mut App, ok: usize, failed: usize) {
    app.push = None;
    if failed == 0 {
        app.set_status(format!("pushed {ok} change{} ✓", if ok == 1 { "" } else { "s" }));
    } else {
        app.set_error(format!("pushed {ok}, failed {failed} — see day view"));
    }
}
