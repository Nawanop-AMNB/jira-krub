# jira-krub

A terminal app for the daily Jira worklog chore.

Every workday has to end with 8h logged in Jira Cloud. Doing that through the
web dialog, ticket by ticket, is slow and easy to forget. jira-krub keeps the
whole routine in one keyboard-driven screen: see which days are short, stage
entries through the day, push them in one batch, and jump to the tickets
assigned to you without opening a browser tab first.

Single user, single Jira site, native worklogs (no Tempo). Spec: [SPEC.md](SPEC.md).

## Install

No Rust needed. macOS (Apple Silicon / Intel) and Linux (x86_64 / arm64):

```bash
curl -fsSL https://raw.githubusercontent.com/Nawanop-AMNB/jira-krub/main/install.sh | sh
```

Puts `jira-krub` and the short alias `jrk` in `~/.local/bin`. Rerun the same
line to upgrade. `JIRA_KRUB_VERSION=v0.3.0` pins a release,
`JIRA_KRUB_INSTALL=/some/dir` changes the target.

From source: `cargo install --path .` (installs both names).

## First run

```bash
jrk
```

The connect screen asks for the Jira site (`https://company.atlassian.net`,
custom domains fine), your email and an API token
(https://id.atlassian.com/manage-profile/security/api-tokens).
`Test connection` calls `/myself`; `Save & start` writes the config and opens
the main screen. `JIRA_API_TOKEN` in the environment overrides the saved token.

Files:

| what | where |
|---|---|
| config (mode 600) | `~/.config/jira-krub/config.toml` — `[connection]`, `[global]`, `[year.YYYY]` |
| local state (staged entries, watchlist) | `~/.local/share/jira-krub/state.json` |

## Screens

### Main screen — two tabs

`Tab` switches between them. The app opens on **My Tasks**.

**My Tasks** — everything assigned to you that is not done, grouped by Jira
status (in-progress statuses first, then to-do). Read-only: it is for finding
work, not logging it.

```
  My Tasks   Worklogs                                                Nawanop K.

┌ my tasks · 8 ────────────────────────────────────────────────────────────────┐
│ 🔍  type to filter                                                           │
│ ─ In Progress · 3 ───────────────────────────────────────────────────────── │
│▶  KAN-12    Fix auth token expiry               due Fri 19 Sep  updated today│
│   KAN-9     Refactor worklog sync               due today       updated 1d ago│
│ ─ To Do · 4 ─────────────────────────────────────────────────────────────── │
│   KAN-14    Migrate config                      overdue 2d      updated 2d ago│
└──────────────────────────────────────────────────────────────────────────────┘
 Enter open in Jira · ↑↓ move · / filter · r sync · Tab worklogs · , settings · q quit
```

`Enter` opens the issue in your browser (`<site>/browse/KEY`). `↑` from the
first row lands in the filter box; typing filters at once, `Ctrl+U` clears.
Due dates show as `due today`, `due Sat 19 Sep` (this week), `due 30 Sep`, or
`overdue Nd`; overdue and today are highlighted.

**Worklogs** — one row per day of the week: progress bar (`█` pushed, `▓`
staged), pushed hours, status, `+Xh staged`. Green means Jira has it; staged
work never counts toward the target, so a forgotten push cannot look done.

```
  My Tasks   Worklogs                                                Nawanop K.

 jira-krub   ◀ 07 Sep   [ 14 Sep → 20 Sep ]   21 Sep ▶
┌ week ────────────────────────────────────────────────────────────────────────┐
│   Mon 14   ████████████████████████  8h        ✓                             │
│ ▶ Wed 16   ██████████████████░░░░░░  6h15      need 1h45      +2h staged     │
│   Thu 17   ░░░░░░░░░░░░░░░░░░░░░░░░  0         empty                         │
│   Fri 18   ────────────────────────  –         future                        │
│   Sat 19   off · Songkran                                                    │
└──────────────────────────────────────────────────────────────────────────────┘
 Enter open day · a add · p push · ←→ week · t today · r sync · Tab tasks · , settings · q quit
```

`Enter` opens the day, `a` stages an entry through the popup, `p` pushes the
week, `←→` move a week, `t` jumps to today. Holidays and non-workdays show
`off` and drop out of the weekly target.

### Day view — tickets | prepare logwork

Left: your tickets (watchlist, assigned, recently logged) with a search box.
Right: the entries staged for that day, plus what is already in Jira.

- Tickets: type to filter locally, Jira search after 300 ms for ≥ 2 chars.
  `Space` / double-click / drag stages 1h at 09:00 (both configurable),
  `Enter` opens the popup with start, duration, title and description,
  `w` toggles the watchlist.
- Prepare: `Enter` walks the row cell by cell (start → duration →
  description), or jump with `s` / `d` / `n`. First key replaces, `↑↓` step
  15 min, `Esc` reverts, `⌫`/`Del` remove, `p` pushes the day.
  Description is multi-line: `Ctrl+Enter` / `Shift+Enter` in terminals with
  the Kitty keyboard protocol (Ghostty, kitty, WezTerm, iTerm), `Ctrl+J`
  anywhere.
- Rows already in Jira (below the `in jira` divider) are editable: an edit
  marks them `~`, `⌫` marks `✗` for deletion (again to undo). The next push
  sends them as PUT / DELETE in the same batch.
- `[` `]` previous / next day, `←` `→` switch pane, `Esc` steps back.

Durations use Jira grammar: `2h`, `1h30m`, `90m`, `1d`. Unit required, no
decimals, snapped to 15 min. Worklogs are sent with `adjustEstimate=leave`,
so remaining estimates are never touched.

### Settings (`,`)

`Tab` switches **Global** (hours per day, workdays, default start, quick-stage
duration, history lookback, auto-watch, Jira connection) and **Year** (`[` `]`
or `←→` pick the year; target override, public holidays). `↑↓` walk fields,
`Enter` opens a field, `Enter` commits, `Esc` reverts, `Ctrl+S` saves.

### Key conventions

- `Tab` only ever switches tabs.
- `↑↓` walk rows; `Enter` opens a field for typing and commits it; `Esc`
  reverts one step. Search and filter boxes are the exception: they sit on
  the top row of their list, `↑` walks into them, typing filters at once.
- `←→` never change the day inside the day view.
- `q` quits; with staged entries it asks first. `Ctrl+C` twice force-quits.
- Mouse works everywhere: click rows, cells, tabs, arrows and footer hints;
  wheel scrolls; drag a ticket onto the prepare pane. Native text selection
  needs Shift+drag.
- Thai, CJK and other non-ASCII text is measured in terminal columns, so the
  cursor and columns stay aligned.

## How it talks to Jira

- Sync is windowed: the visible week plus the current week, minus a
  configurable lookback. Two JQL searches (assigned-to-me, and my worklogs in
  the window) with embedded worklogs, plus one request per watchlist ticket.
  Roughly five to six requests regardless of ticket age. The assigned list
  is handed to the UI as soon as its search returns (My Tasks fills in
  under a second); the watchlist and history legs then run concurrently,
  at most four requests in flight.
- Push sends `POST` / `PUT` / `DELETE` per entry, keeps going after a failure
  and shows the Jira error inline on the row. Only the failed rows retry.
- Rows are reconciled against Jira after every sync, so edits made in the web
  UI show up too.

## Architecture

Clean architecture, feature-first TUI:

```
src/domain/          pure rules, no IO: Entry, Ledger (entries + watchlist keys),
                     Week / DaySummary / WorkCalendar, Issue (+ due / updated
                     labels), duration, StartTime, SiteUrl parsing.
src/application/     ports (JiraGateway, StateStore, ConfigStore, UrlOpener),
                     Config, and use cases: sync, push, search, test_connection.
src/infrastructure/  adapters: HttpJiraGateway (reqwest, REST v3), JsonStateStore,
                     TomlConfigStore, SystemUrlOpener, Worker (threads + mpsc).
src/tui/             ratatui shell: App (state + dispatch), Action enum,
                     HitRegistry (mouse hit-testing), widgets (TextInput, tab
                     bar, width-aware text helpers) and one folder per feature
                     under features/: connect, week, tasks, day, entry_form,
                     push, settings — each split into model / update / view.
src/main.rs          composition root.
```

Dependencies point inward only: `tui → application → domain` and
`infrastructure → application → domain`. Persistence goes through DTOs, so the
domain never derives serde. The TUI never spawns processes or touches the
network directly; it goes through the ports.

## Development

```bash
cargo test                                  # ~270 tests: domain, adapters, TUI reducers
cargo clippy --all-targets -- -D warnings
cargo run --release
```

TUI tests drive a real `App` against a fake gateway and render into a test
backend, so key bindings, layout and mouse hit-testing are covered without a
terminal. A plain `http://localhost` / `http://127.0.0.1` site is accepted so a
fake Jira can be used for manual runs.

Releases: push a `v*` tag and `.github/workflows/release.yml` builds the four
binaries and attaches them to the GitHub release the installer reads.
