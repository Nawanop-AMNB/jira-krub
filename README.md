# jira-krub

Terminal UI for the daily Jira worklog chore. Stage entries through the day,
push them to Jira Cloud in one keypress, and see at a glance which days are
still short of 8h. Spec: [SPEC.md](SPEC.md).

## Run

```bash
cargo run --release
```

First launch opens the connect screen: Jira site (`https://company.atlassian.net`),
email, API token (https://id.atlassian.com/manage-profile/security/api-tokens).
`Test connection` calls `/myself`; `Save & start` writes
`~/.config/jira-krub/config.toml` (mode 600, sections `[connection]`, `[global]`,
`[year.YYYY]`) and opens the week view.
`JIRA_API_TOKEN` in the environment overrides the saved token.

Local state (staged entries, watchlist) lives in
`~/.local/share/jira-krub/state.json`.

## Screens

**Main** — one row per day: bar, pushed hours, status (`✓` / `need 1h` / `empty` /
`off · holiday`), `+Xh staged`. `Enter` open day · `a` add via popup · `p` push week ·
`←→` week · `t` today · `r` sync · `,` settings · `q` quit.

**Settings** (`,`) — `Tab` switches the Global tab (target hours/day, workdays,
default start, quick-stage duration, history lookback, auto-watch, Jira
connection) and the Year tab (`[` `]` pick year; target override, public
holidays). `↑↓` walk fields, `Enter` toggles / edits, `^S` saves. Off days and
holidays show as `off` and drop out of the weekly target.

Key convention: `Tab` only ever switches tabs. `↑↓` walk rows, `Enter` opens
a field for typing (Esc reverts). Search boxes are the exception: they are
the top row of their list, `↑` walks into them and typing filters at once.

**Day view** — tickets on the left, prepare-logwork on the right.
- Tickets: `/` search (local filter instantly, Jira search after 300 ms),
  `Space`/double-click/drag stage 1h at 09:00, `Enter` popup, `w` toggle watchlist. `[`/`]` change day, `←`/`→` switch pane.
- Prepare: `Enter` edits the row cell by cell (start → duration → description,
  `Enter` again leaves), or jump with `s` / `d` / `n`. First key replaces, `↑↓`
  step 15 min, `Esc` reverts, `⌫`/`Del` remove, `p` push day. Descriptions are
  multi-line (one Jira paragraph per line): `Ctrl+Enter` / `Shift+Enter` where
  the terminal supports the Kitty keyboard protocol (Ghostty, kitty, WezTerm,
  iTerm), `Ctrl+J` anywhere.
  Rows already in Jira (below the `in jira` divider) are editable too: an edit
  marks them `~`, Backspace marks `✗` for deletion (again to undo); the change
  is sent as PUT/DELETE on the next push.
- `[` `]` previous/next day. `Esc` steps back: cell → pane → tickets → main.

Durations use Jira grammar: `2h`, `1h30m`, `90m`, `1d`. No decimals, unit required,
snapped to 15 min. Every worklog is sent with `adjustEstimate=leave`.

Mouse: click rows, cells, arrows and footer hints; wheel scrolls; drag a ticket
onto the right pane to stage it. Native text selection needs Shift+drag.

## Architecture

Clean architecture, feature-first UI:

```
src/domain/          pure rules: Entry, Ledger (entries + watchlist), Week/DaySummary,
                     duration + StartTime + SiteUrl parsing. No IO. Unit-tested.
src/application/     ports (JiraGateway, StateStore, ConfigStore) and use cases:
                     sync, push, search, test_connection.
src/infrastructure/  adapters: HttpJiraGateway (reqwest, REST v3), JsonStateStore,
                     TomlConfigStore, Worker (background threads + mpsc).
src/tui/             ratatui shell: App (state + dispatch), Action enum, HitRegistry
                     (mouse hit-testing), TextInput widget, and one folder per
                     feature under features/: setup, week, day, entry_form, push —
                     each with model / update (keys + logic) / view.
src/main.rs          composition root.
```

Dependencies point inward only: `tui → application → domain`, and
`infrastructure → application → domain`. Persistence uses DTOs so the domain
never derives serde.

## Development

```bash
cargo test            # domain + adapter unit tests
cargo clippy --all-targets
```

A plain `http://localhost` / `http://127.0.0.1` site is accepted so a fake Jira
can be used for manual testing.

## Install (no Rust needed)

```bash
curl -fsSL https://raw.githubusercontent.com/Nawanop-AMNB/jira-krub/main/install.sh | sh
```

Downloads the prebuilt binary for macOS (Apple Silicon / Intel) or Linux
(x86_64 / arm64) from the latest GitHub release into `~/.local/bin`.
Releases are built by `.github/workflows/release.yml` on every `v*` tag.
