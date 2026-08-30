# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0](https://github.com/scriptor-pro/memo-tori-gtk/compare/memo-tori-gtk-v0.1.1...memo-tori-gtk-v0.2.0) (2026-08-30)


### Features

* add Alt+Up/Down keyboard equivalent for note reordering ([6be9b5c](https://github.com/scriptor-pro/memo-tori-gtk/commit/6be9b5ca34220c8b49c89a90ecdf81b1f7c44445))
* add drag-and-drop manual reordering of notes ([8e9a81c](https://github.com/scriptor-pro/memo-tori-gtk/commit/8e9a81c8b71f6ae320a2b32630ecf082620ac20b))
* add move_note for manual note reordering ([463a286](https://github.com/scriptor-pro/memo-tori-gtk/commit/463a286235c0ab66690142005b2dba92a7b4e607))
* add notes.position column with recency-based backfill migration ([d621daf](https://github.com/scriptor-pro/memo-tori-gtk/commit/d621daf9b2035376d76445faf1279063010f762a))
* add system tray, single-instance mode, and embedded Nebula Sans font ([26b8958](https://github.com/scriptor-pro/memo-tori-gtk/commit/26b895891b26c69a0dcd0164f77dd9190ca71b9a))
* add visible labels above all input fields for accessibility ([1ab3701](https://github.com/scriptor-pro/memo-tori-gtk/commit/1ab3701dc93641a41c54e3b139aa8d0ec84e34d2))
* apply config.text_scale to the UI font size ([1ba3717](https://github.com/scriptor-pro/memo-tori-gtk/commit/1ba3717185824735c41b2c776e0ded347243d4f8))
* bootstrap Memo-Tori GTK application ([1009816](https://github.com/scriptor-pro/memo-tori-gtk/commit/10098163427adecfedf3617a72c77f289b7ba73b))
* merge tag and content save actions into a single Enregistrer button ([efae854](https://github.com/scriptor-pro/memo-tori-gtk/commit/efae854a823e4cf3cd4019919237d4895a5a25d4))
* new notes get the lowest position so they appear first in manual order ([52ae9ed](https://github.com/scriptor-pro/memo-tori-gtk/commit/52ae9ed932fb9a6e20dec55fc84f733000f17249))
* replace PopoverMenuBar and custom switcher wrap with a native HeaderBar ([a761e3e](https://github.com/scriptor-pro/memo-tori-gtk/commit/a761e3e2f4766e12afc43d2df25e533aa7a9b2e5))
* show a visible keyboard shortcuts hint in the capture panel ([efa3320](https://github.com/scriptor-pro/memo-tori-gtk/commit/efa332063030bc98b92bce8183f3c62af5aa6e26))
* show visible, screen-reader-announced errors on save failures ([197a4e8](https://github.com/scriptor-pro/memo-tori-gtk/commit/197a4e82c087b7ffd67f827c31fb42fa5cf025e9))
* simplify capture panel wording and bolden tab switcher ([7ae61aa](https://github.com/scriptor-pro/memo-tori-gtk/commit/7ae61aac60af5b2644fa4765ca7b559b1d58f688))
* sort notes by manual position when no search or tag filter is active ([6b818f6](https://github.com/scriptor-pro/memo-tori-gtk/commit/6b818f6204ed482c9a62069e561bc84cf171518e))
* translate remaining UI strings to French ([6d0a8db](https://github.com/scriptor-pro/memo-tori-gtk/commit/6d0a8dbe2c17a3b25c03e26b43d750ce85252a25))


### Bug Fixes

* correct multi-step drag reorder loop to follow the dragged note ([7fb4dd6](https://github.com/scriptor-pro/memo-tori-gtk/commit/7fb4dd694fb000f3645338747162a4d34ea004c0))
* translate remaining "Tags: error" string and normalize French colon spacing ([1ce8579](https://github.com/scriptor-pro/memo-tori-gtk/commit/1ce85796b8fc6e9668caafec52eeaaef221e7145))
* translate remaining note-loading error strings to French ([6c10a60](https://github.com/scriptor-pro/memo-tori-gtk/commit/6c10a60e49abbba98da8b1222f92ad79f11903c9))

## 0.2.0 (2026-08-30)

### Features

- system tray, single-instance mode, and embedded Nebula Sans font
- fixed-weight font in note-taking
- manual note reordering (drag-and-drop and Alt+Up/Down keyboard equivalent)
- native HeaderBar replacing PopoverMenuBar and custom switcher
- merged tag and content save actions into a single "Enregistrer" button
- full French translation of the UI
- visible labels on all input fields, visible/announced error banner, applied `text_scale`, and visible keyboard shortcuts hint (WCAG/RGAA accessibility overhaul)

### Fixes

- multi-step drag reorder now follows the dragged note correctly

## 0.1.1 (2026-02-17)

### Features

- bootstrap GTK app with quick capture, search/reader, and Debian packaging script
