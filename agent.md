# Memo-Tori GTK

## Agent Specification

Version: 0.1.0001 Date: 17-02-2026 License: Apache-2.0

---

## 1. Vision

Memo-Tori GTK is a native Linux desktop application designed for
ultra-fast thought capture. It is minimalist, robust, always ready, and
built for long-term maintainability.

Target environment: - Linux only - XFCE-first - Compatible with KDE and
GNOME

Technology stack: - Rust - GTK 4 (without mandatory libadwaita
dependency) - SQLite (with FTS5) - Freedesktop.org standards

---

## 2. Architecture Overview

### Core Principles

- Single-instance application
- Background-ready (tray-enabled)
- Fast startup
- Minimal CPU usage while idle
- Strict XDG compliance
- WCAG 2.1 AA accessibility target

---

## 3. Application Lifecycle

### Default Behavior

- Closing the main window minimizes to tray (if supported).
- Explicit "Quit" action available via tray or menu.
- Preference option: "Quit when closing window" (disabled by default).
- If tray not supported → closing quits application.

---

## 4. Data Storage

### Location (XDG compliant)

Database: \~/.local/share/memo-tori/memo-tori.db

Config: \~/.config/memo-tori/config.toml

---

### SQLite Schema (MVP)

Tables:

notes - id (UUID) - content (TEXT) - created_at (TIMESTAMP) - updated_at
(TIMESTAMP) - deleted_at (TIMESTAMP NULL) - pinned (BOOLEAN)

tags - id (INTEGER PRIMARY KEY) - name (TEXT UNIQUE)

notes_tags - note_id (UUID) - tag_id (INTEGER)

FTS5 virtual table for full-text search.

Soft delete implemented via `deleted_at`.

No encryption in V1.

---

## 5. Quick Capture Window

- Small dedicated window
- Multiline text field
- Auto-focus on open
- Enter: save
- Esc: cancel
- No menus
- Minimal chrome
- Notification on successful save

---

## 6. System Integration

- Freedesktop notifications (libnotify)
- StatusNotifierItem tray support
- Raccourcis configured by desktop environment
- Optional autostart via XDG autostart .desktop entry

---

## 7. Accessibility

- Full keyboard navigation
- Screen reader compatible (AT-SPI)
- High contrast compatibility
- Adjustable text size
- Animations minimal and optional

Target: WCAG 2.1 AA compliance

---

## 8. Performance Goals

- Startup under 300ms (target)
- Minimal idle CPU usage
- Single persistent SQLite connection
- No background polling loops

---

## 9. Versioning

Semantic Versioning (SemVer): MAJOR.MINOR.PATCH (example: 0.1.1)

Single source of truth: `Cargo.toml` package version.

Version displayed in: - About dialog - CLI flag (--version)

---

## 10. Roadmap

### MVP

- SQLite schema
- FTS5 search
- Quick capture window
- Tray integration
- Preferences toggle
- Soft delete

### Future (V2+)

- Corbeille UI
- Advanced search filters
- Export (Markdown)
- Optional encryption
- Tag management UI enhancements

---

End of specification.
