#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PKG_NAME="memo-tori-gtk"
APP_ID="io.github.memo_tori.gtk"
BUILD_DIR="$ROOT_DIR/target/deb-build"
DIST_DIR="$ROOT_DIR/dist"

if ! command -v dpkg-deb >/dev/null 2>&1; then
  echo "dpkg-deb is required to build a .deb package" >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required to build this project" >&2
  exit 1
fi

VERSION="$(awk -F '"' '/^version = / { print $2; exit }' "$ROOT_DIR/Cargo.toml")"
ARCH="$(dpkg --print-architecture)"
PKG_DIR="$BUILD_DIR/${PKG_NAME}_${VERSION}_${ARCH}"

rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/applications"
mkdir -p "$PKG_DIR/usr/share/icons/hicolor/scalable/apps"
mkdir -p "$PKG_DIR/usr/share/icons/hicolor/128x128/apps"
mkdir -p "$DIST_DIR"

echo "Building release binary..."
cargo build --release --manifest-path "$ROOT_DIR/Cargo.toml"

install -m 0755 "$ROOT_DIR/target/release/$PKG_NAME" "$PKG_DIR/usr/bin/$PKG_NAME"

install -m 0644 "$ROOT_DIR/assets/io.github.memo_tori.gtk.desktop" \
  "$PKG_DIR/usr/share/applications/${APP_ID}.desktop"
sed -i "s|^Exec=.*|Exec=$PKG_NAME|" "$PKG_DIR/usr/share/applications/${APP_ID}.desktop"
sed -i "s|^Icon=.*|Icon=memo-tori|" "$PKG_DIR/usr/share/applications/${APP_ID}.desktop"

install -m 0644 "$ROOT_DIR/assets/icons/hicolor/scalable/apps/memo-tori.svg" \
  "$PKG_DIR/usr/share/icons/hicolor/scalable/apps/memo-tori.svg"

if command -v rsvg-convert >/dev/null 2>&1; then
  rsvg-convert -w 128 -h 128 "$ROOT_DIR/assets/icons/hicolor/scalable/apps/memo-tori.svg" \
    > "$PKG_DIR/usr/share/icons/hicolor/128x128/apps/memo-tori.png"
fi

cat > "$PKG_DIR/DEBIAN/control" <<EOF
Package: $PKG_NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: Memo-Tori Contributors <noreply@example.com>
Depends: libc6, libgcc-s1, libgtk-4-1, libglib2.0-0, libnotify4
Description: Ultra-fast thought capture app for Linux (GTK4 + SQLite)
 Memo-Tori GTK is a minimalist desktop app focused on quick idea capture,
 full-text search, and fast note reading.
EOF

DEB_PATH="$DIST_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"
dpkg-deb --build "$PKG_DIR" "$DEB_PATH"

echo "Built package: $DEB_PATH"
