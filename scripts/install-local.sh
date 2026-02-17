#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ICON_SRC="$ROOT_DIR/assets/icons/hicolor/scalable/apps/memo-tori.svg"
DESKTOP_SRC="$ROOT_DIR/assets/io.github.memo_tori.gtk.desktop"
DESKTOP_TARGET="$HOME/.local/share/applications/io.github.memo_tori.gtk.desktop"

APP_BIN="memo-tori-gtk"
if [ -x "$ROOT_DIR/target/debug/memo-tori-gtk" ]; then
  APP_BIN="$ROOT_DIR/target/debug/memo-tori-gtk"
fi

mkdir -p "$HOME/.local/share/icons/hicolor/scalable/apps"
mkdir -p "$HOME/.local/share/icons/hicolor/128x128/apps"
mkdir -p "$HOME/.local/share/icons/hicolor/64x64/apps"
mkdir -p "$HOME/.local/share/icons/hicolor/48x48/apps"
mkdir -p "$HOME/.local/share/pixmaps"
mkdir -p "$HOME/.local/share/applications"

install -m 0644 "$ICON_SRC" \
  "$HOME/.local/share/icons/hicolor/scalable/apps/memo-tori.svg"

if command -v rsvg-convert >/dev/null 2>&1; then
  rsvg-convert -w 128 -h 128 "$ICON_SRC" > "$HOME/.local/share/icons/hicolor/128x128/apps/memo-tori.png"
  rsvg-convert -w 64 -h 64 "$ICON_SRC" > "$HOME/.local/share/icons/hicolor/64x64/apps/memo-tori.png"
  rsvg-convert -w 48 -h 48 "$ICON_SRC" > "$HOME/.local/share/icons/hicolor/48x48/apps/memo-tori.png"
  install -m 0644 "$HOME/.local/share/icons/hicolor/128x128/apps/memo-tori.png" \
    "$HOME/.local/share/pixmaps/memo-tori.png"
fi

install -m 0644 "$DESKTOP_SRC" "$DESKTOP_TARGET"
sed -i "s|^Icon=.*|Icon=$HOME/.local/share/icons/hicolor/128x128/apps/memo-tori.png|" "$DESKTOP_TARGET"
sed -i "s|^Exec=.*|Exec=$APP_BIN|" "$DESKTOP_TARGET"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$HOME/.local/share/applications" || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q "$HOME/.local/share/icons/hicolor" || true
fi

if command -v xdg-icon-resource >/dev/null 2>&1; then
  xdg-icon-resource forceupdate --theme hicolor || true
fi

echo "Installed desktop entry and icon for current user."
