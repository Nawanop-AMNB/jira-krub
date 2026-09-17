#!/bin/sh
# jira-krub uninstaller — removes what install.sh put on this machine.
#
#   curl -fsSL https://raw.githubusercontent.com/Nawanop-AMNB/jira-krub/main/uninstall.sh | sh
#
# By default only the binary and the `jrk` alias are removed; your config
# (site, email, API token) and local state (staged entries, watchlist) are
# kept so a later reinstall picks up where you left off. To remove those too:
#
#   curl -fsSL .../uninstall.sh | sh -s -- --purge
#
# Options:
#   --purge             also delete config and state
#   JIRA_KRUB_INSTALL   directory the binary was installed to (default: ~/.local/bin)
#   XDG_CONFIG_HOME / XDG_DATA_HOME are honoured like the app does
set -eu

BIN="jira-krub"
ALIAS="jrk"
INSTALL_DIR="${JIRA_KRUB_INSTALL:-$HOME/.local/bin}"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/$BIN"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/$BIN"

say() { printf '%s\n' "$*" >&2; }

purge=0
for arg in "$@"; do
  case "$arg" in
    --purge) purge=1 ;;
    -h|--help) sed -n '2,15p' "$0" 2>/dev/null || say "usage: uninstall.sh [--purge]"; exit 0 ;;
    *) say "error: unknown option $arg (only --purge is accepted)"; exit 1 ;;
  esac
done

removed=0
remove() {
  # $1 = path, $2 = label
  if [ -e "$1" ] || [ -L "$1" ]; then
    rm -rf "$1"
    say "removed $2: $1"
    removed=$((removed + 1))
  fi
}

remove "$INSTALL_DIR/$ALIAS" "alias"
remove "$INSTALL_DIR/$BIN" "binary"

# a cargo-installed copy lives elsewhere; say so rather than guess
if command -v "$BIN" >/dev/null 2>&1; then
  say "note: another $BIN is still on PATH at $(command -v "$BIN") — not installed by install.sh, remove it by hand (cargo uninstall $BIN if cargo put it there)"
fi

if [ "$purge" -eq 1 ]; then
  remove "$CONFIG_DIR" "config"
  remove "$DATA_DIR" "state"
else
  for d in "$CONFIG_DIR" "$DATA_DIR"; do
    [ -d "$d" ] && say "kept $d (rerun with --purge to delete)"
  done
fi

if [ "$removed" -eq 0 ]; then
  say "nothing to remove: $BIN was not installed in $INSTALL_DIR"
else
  say "done"
fi
