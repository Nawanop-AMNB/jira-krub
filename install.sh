#!/bin/sh
# jira-krub installer — downloads the prebuilt binary for this machine.
#
#   curl -fsSL https://raw.githubusercontent.com/Nawanop-AMNB/jira-krub/main/install.sh | sh
#
# Options (env vars):
#   JIRA_KRUB_VERSION   tag to install, e.g. v0.2.0 (default: latest release)
#   JIRA_KRUB_INSTALL   directory for the binary (default: ~/.local/bin)
set -eu

REPO="Nawanop-AMNB/jira-krub"
BIN="jira-krub"
ALIAS="jrk"
VERSION="${JIRA_KRUB_VERSION:-latest}"
INSTALL_DIR="${JIRA_KRUB_INSTALL:-$HOME/.local/bin}"

say() { printf '%s\n' "$*" >&2; }
die() { say "error: $*"; exit 1; }

os="$(uname -s)"; arch="$(uname -m)"
case "$os:$arch" in
  Darwin:arm64)                target="aarch64-apple-darwin" ;;
  Darwin:x86_64)               target="x86_64-apple-darwin" ;;
  Linux:x86_64)                target="x86_64-unknown-linux-musl" ;;
  Linux:aarch64|Linux:arm64)   target="aarch64-unknown-linux-musl" ;;
  *) die "no prebuilt binary for $os/$arch — build from source: cargo install --git https://github.com/$REPO" ;;
esac

if [ "$VERSION" = "latest" ]; then
  url="https://github.com/$REPO/releases/latest/download/$BIN-$target.tar.gz"
else
  url="https://github.com/$REPO/releases/download/$VERSION/$BIN-$target.tar.gz"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say "downloading $url"
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$url" -o "$tmp/$BIN.tar.gz" || die "download failed (no release for $target yet?)"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$tmp/$BIN.tar.gz" "$url" || die "download failed (no release for $target yet?)"
else
  die "need curl or wget"
fi

tar -xzf "$tmp/$BIN.tar.gz" -C "$tmp"
[ -f "$tmp/$BIN" ] || die "archive did not contain $BIN"

mkdir -p "$INSTALL_DIR"
install -m 755 "$tmp/$BIN" "$INSTALL_DIR/$BIN"
ln -sf "$BIN" "$INSTALL_DIR/$ALIAS"
say "installed $INSTALL_DIR/$BIN ($("$INSTALL_DIR/$BIN" --version 2>/dev/null || echo "$VERSION")) · short alias: $ALIAS"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    say ""
    say "add to PATH (then open a new shell):"
    say "  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.zshrc"
    ;;
esac
say ""
say "run:  $BIN  or  $ALIAS   (first launch opens the setup screen)"
