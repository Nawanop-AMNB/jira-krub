# jira-krub — Product Spec

**Status:** draft v0.6 · 2026-09-15
**Owner:** ka.nawanop (sole user)
**Target:** Jira Cloud (`*.atlassian.net`), native worklog dialog (no Tempo)

---

## 1. Problem Statement

Company policy requires every workday to carry **at least 8h of Jira worklogs**.
Logging through the Jira web UI means opening each ticket, clicking "Log work",
filling a modal, and repeating per ticket per day. It is slow enough that it
gets deferred, and by Friday several days are empty and must be
reconstructed from memory, git history, and the calendar.

Cost of not solving: 5–15 min/day of clicking, weekly scramble to backfill,
inaccurate logs, and the low-grade stress of knowing days are short.

## 2. Goals

| # | Goal | Measure |
|---|------|---------|
| G1 | Capture a unit of work in **under 10 seconds** without leaving the terminal | Stopwatch on `add entry` flow |
| G2 | **Zero workdays under 8h** at end of week | Week view shows no red days |
| G3 | Batch-push a full day or week of entries in **one action** | One keypress → N worklogs created |
| G4 | Backfill a forgotten day in **under 2 minutes** using git hints | Timed on a real forgotten day |
| G5 | Never leave the terminal in a broken state | Raw mode always restored, including on panic |

## 3. Non-Goals (v1)

- **Tempo support.** Company uses native Jira worklogs. Keep the Jira client
  behind a trait so Tempo can be added later, but do not build it.
- **Multi-user / team distribution.** Single user, single machine, single token.
- **Jira issue management.** No creating, editing, deleting, transitioning,
  or assigning issues. The tool only reads issues and writes worklogs. Tickets
  that are not mine are handled by the local watchlist (R2), not by changing
  anything in Jira.
- **Time tracking timers.** Entries are typed after the fact.
- **Remaining-estimate management.** Company does not use it; worklogs are sent
  with `adjustEstimate=leave` and the field is never shown.
- **Holiday calendar.** Only weekends are treated as non-workdays. No holiday
  config or built-in calendar.
- **Jira Server / Data Center.**

## 4. Persona

**The developer** — works across 2–5 tickets a day, sits in a terminal, has
meetings that also count toward the 8h, and sometimes logs against tickets
that are assigned to someone else (shared, ops, meeting tickets).

## 5. User Stories

Ordered by priority.

**Issue list**
- As the developer, I want the grid to show all my un-done assigned tickets by
  default, so that the things I am likely to log against are already there.
- As the developer, I want to add any ticket to a watchlist by typing its key
  (e.g. `STW-123`), even if it is not assigned to me, so that shared or
  standing tickets (meetings, support, ops) stay in the grid.
- As the developer, I want to log work against a watched ticket exactly like
  my own tickets, so that shared tickets count toward my 8h.
- As the developer, I want to remove a ticket from the watchlist, so that the
  grid only shows what I still log against.

**Capture**
- As the developer, I want to jot "1h30 on ABC-123: fixed auth bug" the moment
  I finish, so that I do not have to remember it later.
- As the developer, I want each entry to have a short title and an optional
  longer detail, so that the Jira comment is meaningful without extra typing.
- As the developer, I want to pick the ticket by fuzzy-typing part of its key
  or summary, so that I never type a full key.
- As the developer, I want a start time that defaults to 09:00 but can be
  nudged or typed, so that the Jira timeline looks plausible when it matters.

**Review and batch push**
- As the developer, I want to see the whole week as a grid of ticket × day with
  hours per cell, so that I can spot short days instantly.
- As the developer, I want to see how many hours a day still needs to reach 8h,
  so that I can fill the gap precisely.
- As the developer, I want to edit or delete a local entry before it is sent,
  so that I can fix mistakes cheaply.
- As the developer, I want to push all pending entries for a day or a week in
  one action, so that logging is a single end-of-day habit.
- As the developer, I want a clear pushed / pending / failed state per entry,
  so that I never double-log or silently lose one.

**Backfill**
- As the developer, I want to log against any past date, so that I can fill a
  missed day.
- As the developer, I want the tool to suggest tickets for a past day from my
  git commits and branch names, so that I do not have to remember.
- As the developer, I want to repeat yesterday's entries onto today (or a
  standing entry like "daily standup 15m"), so that recurring work is free.

**Edge cases**
- As the developer, I want a Jira error (401, 404, network) to be shown with
  the failing entry kept as pending, so that I can retry without retyping.
- As the developer, I want the app to work read-only when offline (last known
  state), so that I can still queue entries.
- As the developer, I want weekends excluded from the 8h check.
- As the developer, I want a typo'd watchlist key (`STW-99999`) to be rejected
  with a clear message, so that the grid never shows a ghost row.

## 6. Requirements

### P0 — Must have

**R0. First-run setup screen**
- Shown when no config exists, or when startup auth returns 401/403
  (prefilled, with "token rejected"). Network failure on startup does **not**
  open setup; main opens in offline mode with last state.
- Fields (Tab order): Jira site, email, API token; buttons
  `[ Test connection ] [ Save & start ] [ Quit ]`.
- **Site**: free-form URL, custom domains allowed. Rules: must be `https://`
  (typed without scheme → auto-prefixed; `http://` → red "must be https");
  host `[a-z0-9.-]` with at least one dot, optional port; everything after
  the host is stripped (`https://x.atlassian.net/browse/A-1` →
  `https://x.atlassian.net`); trailing slash stripped. Hint line shows
  `e.g. https://company.atlassian.net`.
- **Email**: basic `x@y.z` check.
- **Token**: masked with `•`; paste supported (bracketed paste); `Ctrl+U`
  clears; never echoed or logged. Hint line links to
  `id.atlassian.com/manage-profile/security/api-tokens`.
- **Test connection**: background `GET <site>/rest/api/3/myself`. Result
  line: `✓ signed in as <displayName>` or `✗ 401 unauthorized` /
  `✗ site not found` / `✗ 404 — site reachable but not a Jira Cloud API,
  check URL` / `✗ network`. Offending field turns red.
- **Save & start**: runs the test if not yet green; saves only on `✓`. Writes
  `~/.config/jira-krub/config.toml` with mode `600`, then enters main and
  starts sync. **Quit** writes nothing.
- Settings reachable later from main via `,` — same form, prefilled, token
  shown as `••••` unless retyped.
- AC: paste `https://acme.atlassian.net/jira/software/projects/X` into site,
  valid email and token, Enter → `✓ signed in as …`, config written with
  `base_url = "https://acme.atlassian.net"`, main screen appears.

**R1. Main screen — week summary (days only)**
- One row per day Mon–Sun. No ticket columns on this screen.
- Row shows: day label, progress bar, hours (pushed + staged), status word,
  and staged count. Status: `✓` green ≥ target · `need Xh` yellow · `empty`
  red on past workday · `future` dim · weekend blank.
- Header: `◀ prev-week  [ 14 Sep → 20 Sep ]  next-week ▶`, week total vs
  weekly target, total staged count.
- Keys: `←/→` or `[`/`]` shift week · `t` current week · `j/k` select day ·
  `Enter` open day view · `a` stage entry on selected day · `p` push week.
- AC: with Mon–Tue at 8h pushed and Wed at 6h15 (2h staged), rows read
  `8h ✓`, `8h ✓`, `6h15 need 1h45 · 2 staged`.

**R2. Issue list = "work for me" + watchlist**
- Default JQL: `assignee = currentUser() AND statusCategory != Done`,
  overridable in config.
- **Watchlist**: a local list of issue **keys only** (any assignee, any
  status). Always included in the grid, listed first, marked `*`. Persisted
  in the state file. Titles are never cached locally: they come from the
  Jira search result at the moment of watching and are re-fetched on every
  sync; offline the row shows the key with `(offline)`.
- Watchlist rows are first-class: add entry (R4), day view (R5), and batch
  push (R6) work on them exactly as on assigned issues. Jira allows logging
  work on any issue you can view, regardless of assignee.
- Add: type the key (`STW-123`) or a keyword in the day-view search box
  (R4); `w` on a Jira result adds it, or staging an entry on it adds it
  automatically. The issue is fetched, so unknown keys never get a row.
  Adding a key already in the default list is a no-op with a message.
- Remove: key on a highlighted watchlist row. Only removes from the local
  list; nothing changes in Jira. Existing entries against that issue are kept.
- Issues that have my worklogs in the visible week but are neither assigned
  nor watched still appear (history is never hidden), grouped last and dimmed.
- Order: watchlist → assigned (by updated desc) → other-with-worklogs.
- AC: watch `OPS-7` (assigned to someone else); after restart and refresh it
  is still in the grid at the top with its summary.
- AC: add a 1h entry on `OPS-7` and push; Jira shows the worklog under my
  name and the grid cell shows `1h`.

**R3. Local entry model (drafts)**
- Entry = { id, issue_key, date, start_time, seconds, title, detail?, state,
  jira_worklog_id?, error? }.
- `state ∈ { pending, pushed, failed }`.
- Persisted at `~/.local/share/jira-krub/state.json` (XDG data dir) together
  with the watchlist; written atomically on every change.
- Pushed entries keep their Jira worklog id, are never re-pushed, and can be
  edited/deleted remotely (P1).
- AC: kill the process mid-session; on restart all pending entries and the
  watchlist are present.

**R4. Day view — tickets | prepare logwork**
- Opened with Enter on a day. Title bar: date, staged hours, pushed hours,
  remaining to target. `←/→` move day without leaving the view. Esc → main.
- **Left pane: tickets.** Search box on top, then watchlist rows (`*`), then
  my un-done tickets (R2).
  - Typing in the search box filters the local list instantly.
  - After **300 ms** idle with ≥ 2 chars, a background Jira search runs:
    project-like input (`KAN`) → `project = KAN OR text ~ "KAN*"`;
    key-like input (`KAN-1`) → `key = KAN-1 OR project = KAN`; anything else
    → `text ~ "<q>*"`. Jira's text search never matches keys, so the project
    clause is what makes typing a key prefix work. Unknown project → JQL
    error → silent fallback to text search. Results ranked exact key →
    key prefix → rest, top 5. Results appear in a
    "jira" section below the local rows with a spinner while loading. Stale
    responses (older request id) are dropped. Same query is cached for the
    session. Esc clears the search.
  - `Space` / double-click / drag-to-right = **quick stage**: entry created
    on the right with duration `1h`, start `09:00`, empty title. No popup.
    Focus stays on the tickets pane so several tickets can be staged in a
    row; the prepare cursor is left on the newest row.
    `→` only moves focus to the prepare pane (never stages).
  - `Enter` = stage via popup (R5) when details are known now.
  - Staging a ticket from the "jira" section also adds it to the watchlist.
  - `w` toggles the highlighted ticket on/off the watchlist. Same ticket
    can be staged more than once.
- **Right pane: prepare logwork (staged entries for this day).**
  - Columns: start · duration · ticket · **title** (issue summary from Jira,
    read-only) · **description** (the worklog comment, editable). Pending rows first (marker
    `+` new, `~` edited, `✗` delete, `!` failed), then a `─ in jira ─`
    divider and the worklogs Jira already has, dimmed with `✓`.
  - Rows in Jira are editable too: the cursor moves into them, `s`/`d`/`n`/`e`
    edit → row becomes `~ edited` and moves up to pending; Backspace marks
    `✗ delete` (Backspace again undoes). Nothing touches Jira until `p`.
    A Jira worklog cannot change ticket or date (delete + re-add).
  - Inline edit on the highlighted row (cell becomes an editor, no modal):
    `s` start (type `HH:MM` or `HHMM`, or `↑/↓` step 15 min, `Shift+↑/↓`
    step 1 h) · `d` duration (type Jira grammar, or `↑/↓` step 15 min,
    `Shift+↑/↓` 1 h) · `n` description. `Enter` on a row starts at the
    start cell; `Enter`/`Tab` commit and move start → duration → description,
    and a final `Enter` leaves edit mode. **Leaving the cell in any
    way saves it** — Enter, Tab, clicking another cell/row/pane, changing
    day, pushing. Values stay on the row (and in `state.json`) until the
    batch push. Only `Esc` reverts. Unparseable text on unfocus keeps the
    old value and shows an error in the status line.
  - The popup (R5) is for new entries only (`a` on main, Enter on a ticket).
    Existing rows are edited inline; to change ticket or date, delete + re-add.
  - Backspace or Delete removes the row.
  - Start time is **always `09:00` by default** (config `default_start_time`).
    No auto-stacking; the user sets it by hand when it matters. Overlapping
    entries are allowed and flagged with `!`.
  - Title is optional. Empty title shows `(no title)` dimmed; the worklog is
    pushed without a comment.
- `Tab` switches pane. `p` pushes this day only (R6).
- AC: from tickets pane, `Space` on `ABC-411` then `u` `2h` Enter, `n`
  `fix expiry` Enter produces a staged row `09:00 2h ABC-411 fix expiry`
  and the main screen shows `1 staged` on that day.

**R5. Entry form (popup)**
- Fields, tab order: issue (fuzzy over tickets pane list, prefilled), date
  (`←/→`), start (default `09:00`, `↑/↓` 15 min, typeable), duration, title,
  detail (optional, single line in v1).
- Duration grammar = Jira's: integer + unit, units `w d h m`, any order,
  spaces optional. Valid: `2h`, `1h30m`, `1h 30m`, `90m`, `1d`, `1d 2h`.
  Unit required (`2` rejected), no decimals (`1.5h` rejected), no bare
  trailing number (`1h30` rejected). `d` = 8h, `w` = 5d.
  **Snapped to nearest 15 min, minimum 15 min.** Displayed Jira-style:
  `1h 30m`. Invalid input turns the cell red with hint `e.g. 1h30m, 90m`
  and stays in edit.
- Shows "on this issue today: Xh · day total: Yh · remaining: Zh".
- Enter saves as a staged (`pending`) entry. **No Jira call.** Esc cancels.
- Buttons `[ Save ] [ Cancel ]` at the bottom for mouse.

**R5b. Local entry CRUD**
- Edit and remove work on staged entries from the prepare pane or the popup.
- AC: removing a staged entry updates the day total on main immediately.

**R6. Batch push**
- One key pushes all pending entries in the current week; confirm prompt shows
  count and total hours. Variant: push only the selected day.
- Each pending row → new: `POST …/worklog`, edited: `PUT …/worklog/{id}`,
  delete: `DELETE …/worklog/{id}`, all with `adjustEstimate=leave`; body =
  `timeSpentSeconds`, `started` = date + start_time in local tz, `comment`
  = ADF (title paragraph, detail paragraph if present). Confirm shows
  `push 2 new · 1 edited · 1 delete (Xh)`.
- Background thread; UI stays responsive; progress shown as `3/7`.
- On success entry → `pushed` with worklog id. On failure entry → `failed`
  with error text; push continues with remaining entries.
- AC: with 7 pending and one bad key, 6 become pushed, 1 becomes failed with
  HTTP status visible, and re-pushing sends only the failed one.

**R7. Sync from Jira**
- On start and on `r`: fetch account id, issues (R2), and my worklogs for those
  issues plus any issue with `worklogAuthor = currentUser()` in the visible
  range.
- Jira is the source of truth for pushed rows: on every sync, local
  `pushed` entries take the remote start/duration/comment, and ones Jira no
  longer has are dropped. Rows with a pending edit/delete are left alone.
  Unknown remote worklogs are shown as pushed with no local entry.
- Sync never deletes pending entries or the watchlist.

**R8. Config and auth**
- `~/.config/jira-krub/config.toml` (mode 600):
  `base_url`, `email`, `api_token`, `hours_per_day` (8), `jql` (override),
  `default_start_time` ("09:00").
- Env `JIRA_API_TOKEN` overrides `api_token` when set. Token never logged.
- Config is created by the setup screen (R0), not by hand; hand edits still
  respected.

**R9. Mouse support**
- Enable crossterm mouse capture. Every drawn widget records its `Rect`;
  clicks are hit-tested against those.
- Main: click day row selects, double-click opens; click `◀`/`▶` shifts week;
  click week label = today; click staged count = push confirm; wheel = week.
- Day view: click selects row and focuses pane; double-click on a ticket =
  quick stage; drag ticket → right pane = quick stage; click `◀`/`▶` in
  title = prev/next day; wheel scrolls the pane.
- Prepare pane cells: **single click** on a start / duration / title cell
  enters edit for that cell (same as `s`/`u`/`n`); click inside the cell
  while editing moves the cursor; clicking anywhere else commits the edit
  (another cell then starts editing).
  While editing, `▲`/`▼` glyphs at the cell
  edges are clickable (±15 min) and the wheel over the cell steps too.
  Editable cells are drawn with a dim underline so they read as clickable.
- Popup: click focuses a field; click `←→`/`↑↓` glyphs step; click buttons.
- Footer hints are clickable and act like their key.
- Keyboard remains complete; the mouse never has an action the keyboard lacks.
- Note: with capture on, native text selection needs Shift+drag.

**R9b. Esc = step back, everywhere**

| Where | Esc |
|-------|-----|
| Cell edit | revert, back to row |
| Search box with text | clear text |
| Search box empty and focused | focus ticket list |
| Popup | close, discard |
| Push confirm | no |
| Day view, prepare pane | focus tickets pane |
| Day view, tickets pane | main |
| Settings | main, unsaved |
| Setup (first run) | quit, nothing written |
| Main | nothing; `q` quits (`q` with staged entries asks `N staged not pushed, quit? [y/n]`) |

**R10. Terminal safety**
- Raw mode and alternate screen restored on normal exit, `q`, Ctrl-C, panic.
- Any blocking network call happens off the UI thread.

### P1 — Should have (fast follow)

**R11. Git hints for backfill**
- Config `repos = ["~/code/foo", ...]`. For the selected day, run
  `git log --since --until --author=<email>` in each repo, extract keys
  (`[A-Z][A-Z0-9]+-\d+`) from branch/commit messages, show a "suggested"
  section in the issue picker with commit subjects as title hints.

**R12. Repeat and templates**
- `y` = copy last workday's entries to the selected day as pending.
- Config `templates = [{ issue = "OPS-1", minutes = 15, title = "standup" }]`
  with a key to apply all templates to a day.

**R13. Remote edit/delete of worklogs** — *moved to P0, done (see R4/R6)*
- Local states `modified` / `deleted` carry the Jira worklog id; a failed
  update/delete keeps its intent and is retried on the next push.

**R14. Fill-to-target helper**
- In the entry form, a key sets duration to what is left to reach 8h that day
  (snapped down to 15 min).

**R15. Auto-stack start times (off by default, user explicitly does not want it on)**
- Config flag only. When on, a new entry defaults to the end of the previous
  staged entry instead of 09:00.

### P2 — Future (design for, do not build)

- **Calendar hints**: ICS feed URL → meetings offered as entries against a
  watched "meetings" ticket.
- **Tempo backend** behind a `WorklogBackend` trait.
- **Keychain token storage** via macOS `security` CLI.
- **Watchlist auto-expiry**: drop watched issues once they reach Done and
  have no entries in the last N weeks.

## 6b. UI Reference

Three screens. All mockups at 80 columns.

**Setup — first run**
```
┌ jira-krub · setup ───────────────────────────────────────────────┐
│                                                                  │
│   Jira site   [https://jira.company.com▏                  ]      │
│               e.g. https://company.atlassian.net                 │
│   Email       [you@company.com                            ]      │
│   API token   [••••••••••••••••••••••••••••••••••••••••••]       │
│               id.atlassian.com/manage-profile/security/api-tokens│
│                                                                  │
│   ✓ signed in as Nawanop K.                                      │
│                                                                  │
│               [ Test connection ]   [ Save & start ]   [ Quit ]  │
└──────────────────────────────────────────────────────────────────┘
 Tab next · Shift+Tab back · Enter test/save · Esc quit
```

**Main — week summary**
```
 jira-krub   ◀ 07 Sep   [ 14 Sep → 20 Sep ]   21 Sep ▶     32h / 40h   3 staged
┌──────────────────────────────────────────────────────────────────────────────┐
│   Mon 14   ████████████████████████  8h        ✓                              │
│   Tue 15   ████████████████████████  8h        ✓                              │
│ ▶ Wed 16   ██████████████████░░░░░░  6h15      need 1h45      2 staged        │
│   Thu 17   ░░░░░░░░░░░░░░░░░░░░░░░░  0         empty                          │
│   Fri 18   ────────────────────────  –         future                         │
│   Sat 19                                                                      │
│   Sun 20                                                                      │
└──────────────────────────────────────────────────────────────────────────────┘
 [a add] [Enter open day] [p push week] [← →] week  [t today]  [q quit]
```

**Day view — tickets | prepare logwork**
```
┌ ◀ Wed 16 Sep ▶ · staged 6h · pushed 15m · need 1h45 ─────────────────────────┐
│ tickets                          │ prepare logwork                           │
│ 🔍 supp▏                         │   start  dur   ticket    title            │
│ ─ mine + watchlist ─────────────│ ▶ 09:00  2h    ABC-411   fix expiry check │
│ ▶ * STW-123  Support rotation    │   09:00  4h    ABC-398   merge sync       │
│ ─ jira (2) ─────────────────────│   09:00  15m   OPS-7     (no title)       │
│     STW-140  Support: SSO outage │ ─ pushed ──────────────────────────────── │
│     STW-138  Support tooling     │   09:00  15m   OPS-7     standup       ✓  │
│                                  │                                           │
│ [Space stage] [Enter popup] [w]  │ [s start] [d dur] [n title] [e] [⌫]       │
└──────────────────────────────────────────────────────────────────────────────┘
 [p push day]  [Tab pane]  [← →] day  [Esc main]
```

**Popup — entry form**
```
        ┌ log work ─────────────────────────────────────┐
        │  Issue     ABC-411  Fix auth token expiry      │
        │  Date      Wed 16 Sep        ← →               │
        │  Start     09:00             ↑ ↓ 15m           │
        │  Duration  1h30m▏                              │
        │  Title                                         │
        │  Detail                                        │
        │                                                │
        │  on issue today 2h · day 6h15 · need 1h45      │
        │                    [ Save ]  [ Cancel ]        │
        └────────────────────────────────────────────────┘
```

**Daily flow:** open → Enter on today → `Space` on 2–3 tickets → `u`/`n` fill
each → `p` → `y`. Friday: `[` back a week, red days jump out, fill them.

## 7. Decisions

Resolved from Q&A on 2026-09-15.

| Topic | Decision |
|-------|----------|
| Default issue list | My un-done assigned issues ("work for me") |
| Other tickets | Local **watchlist**: add by key, remove by key. No Jira writes to issues |
| Issue create/edit/delete | Out of scope. Tool is read-only on issues |
| Start time | Always defaults to 09:00; per-entry editable inline or in popup. No auto-stacking |
| Main screen | Days only, one row per day with hours + status. No ticket grid |
| Day view | Two panes: tickets (search + watchlist + mine) → prepare logwork (staged entries) |
| Staging | `→`/Space quick-stage with 1h/09:00/no title; Enter for popup; edit inline on right |
| Search | Local filter instantly; Jira search after 300 ms debounce, ≥ 2 chars, cached |
| Mouse | Full click/drag/wheel support, keyboard-equivalent; single-click edits a cell |
| Esc | Always one step back; never quits from main |
| Duration input | Jira grammar, integers + unit only (`1h30m`, `90m`), no decimals, no bare numbers. Snap to 15 min, min 15 min |
| Inline editing | Start/duration/title edited in-cell on the prepare pane; popup only for date/ticket/detail |
| Remaining estimate | Unused; always `adjustEstimate=leave`, field never shown |
| Holidays | None. Weekends only |
| Storage | Single JSON state file; SQLite not needed at this scale |
| First run | In-app setup screen (site URL any https host, email, masked token, test before save) instead of example-config-and-exit |

## 8. Open Questions

- **Q6 (eng)** — `/rest/api/3/search/jql` availability on the company site.
  If 404, fall back to `/rest/api/3/search`. Verify on first real run.
- **Q7 (user, non-blocking)** — Watchlist rows for issues that reach Done:
  keep showing until removed by hand (assumed), or auto-hide?

## 9. Phasing

| Phase | Scope | Exit criterion |
|-------|-------|----------------|
| 0 (done) | Scaffold | — |
| 1 (done) | R0 setup screen + R1 main screen + R3 state file + R2 watchlist + R4 day view (local filter only) + R5 popup + R5b CRUD | A week can be staged fully offline, watchlist survives restart |
| 2 (done) | R6 batch push + R7 sync merge + R8 config + R10 + R4 Jira search | Real week logged end-to-end, zero web UI |
| 3 (done) | R9 mouse | Everything reachable by click |
| 4 | R11 git hints + R12 repeat/templates + R14 fill | Forgotten day backfilled in < 2 min |
| 5 | R13 remote worklog edit/delete, R15, then P2 as needed | — |

No hard deadline. Phase 2 is the first version worth using daily.

## 10. Implementation status (2026-09-16)

| Req | Status | Notes |
|-----|--------|-------|
| R0 setup | done | any https host (plus loopback http for a fake server), test-before-save, mode 600 |
| R1 main | done | bar shows pushed █ / staged ▓; `N staged` clickable |
| R2 issue list | done | watchlist first, mine, then "logged recently"; Jira search results auto-watch when staged |
| R3 state | done | `state.json`, DTO-mapped, atomic write |
| R4 day view | done | `←/→` = switch pane only; `Space`/double-click/drag stage; `[` `]` = day; `d` = duration, ⌫/Del = remove |
| R5 popup | done | fuzzy suggestions over local issues; Enter on Issue accepts |
| R5b CRUD | done | inline cells, first keystroke replaces |
| R6 push | done | title optional (no comment sent); per-entry failed state with HTTP message inline |
| R7 sync | done | history JQL looks back 3 weeks |
| R8 config | done | |
| R9 mouse | done | click / double / drag / wheel / footer buttons; `▲▼` steppers while editing |
| R9b Esc | done | |
| R10 safety | done | panic hook restores terminal, mouse capture and bracketed paste |
| R13 remote edit/delete | done | adopt Jira-only rows on first edit; `~`/`✗` markers; PUT/DELETE in the same batch |
| R11, R12, R14, R15 | not started | P1 |

Verified end to end against a local fake Jira (setup → sync → search → watch →
stage → inline edit → push with one failure → restart with state intact).
Not yet run against a real Atlassian site.
