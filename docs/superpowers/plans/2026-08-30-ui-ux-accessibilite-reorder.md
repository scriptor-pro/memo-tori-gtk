# Refonte UI/UX, accessibilité et réordonnancement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Simplifier la navigation et les actions de Memo-Tori GTK, mettre l'interface entièrement en français, corriger les manquements WCAG/RGAA identifiés (labels, erreurs visibles, taille de texte, aide raccourcis), et ajouter le réordonnancement manuel des notes par glisser-déposer avec équivalent clavier.

**Architecture:** Le fichier `src/app.rs` (une seule fonction `run()` monolithique construisant toute l'UI) est modifié en place, tâche par tâche, en gardant le style impératif GTK4 déjà utilisé (pas de refactor vers des composants séparés — non demandé, hors périmètre). `src/db.rs` reçoit une migration procédurale (ALTER TABLE conditionnel, SQLite n'a pas de migrations versionnées ici) et une nouvelle fonction de déplacement de note. Aucun framework de test GTK n'existe dans ce projet ; les tâches touchant la logique pure (tri SQL, calcul de position) sont testées par `#[test]` unitaires sur `rusqlite::Connection::open_in_memory()`, les tâches touchant uniquement l'UI GTK sont vérifiées par compilation + lancement manuel outillé (`cargo build`, lancement du binaire, capture de fenêtre ciblée via `xdotool`/`import`, jamais de capture plein écran).

**Tech Stack:** Rust, GTK4 (gtk-rs 0.9), rusqlite (SQLite + FTS5), pas de framework de test GTK.

**Spec:** `docs/superpowers/specs/2026-08-30-ui-ux-accessibilite-reorder-design.md`

## Global Constraints

- Tous les textes visibles par l'utilisateur sont en français (voir table de correspondance dans la spec).
- Aucune régression sur le tray/single-instance déjà en place (`src/tray.rs`, état `main_window` dans `src/app.rs`).
- Toute action déclenchable à la souris (drag & drop) doit avoir un équivalent clavier (WCAG 2.1.1).
- Les couleurs déjà validées AA/AAA dans le CSS existant (`#172127` sur `#EDF5F3`/`#ffffff`, `#496067` sur blanc, etc.) ne changent pas — seules des couleurs nouvellement introduites (bannière d'erreur) doivent être vérifiées au ratio WCAG avant intégration.
- Ne pas casser les bases SQLite existantes : toute évolution de schéma doit être rétrocompatible (colonnes ajoutées avec `DEFAULT`, jamais de `DROP`/renommage destructif).
- Pas de nouvelle dépendance Cargo sans nécessité absolue (le projet a déjà `ksni`, `async-channel`, `tokio` ajoutés récemment ; GTK4 fournit `DragSource`/`DropTarget`/`EventControllerKey` nativement, aucun ajout requis pour cette phase).

---

## File Structure

- **Modify `src/db.rs`** : migration procédurale colonne `position`, fonction `move_note`, tri conditionnel dans `search_notes`.
- **Modify `src/app.rs`** : HeaderBar, fusion des actions, textes français, labels visibles, bannière d'erreur, `text_scale`, aide raccourcis, drag & drop + clavier de réordonnancement. `install_css()` change de signature (prend le facteur d'échelle).
- **No new files** : le projet reste à plat (`src/*.rs`), cohérent avec la structure actuelle.

---

### Task 1: Migration procédurale — colonne `position` sur `notes`

**Files:**
- Modify: `src/db.rs:17-22` (fonction `open_and_init`)
- Test: `src/db.rs` (module `#[cfg(test)]` en bas de fichier, nouveau)

**Interfaces:**
- Consumes: `rusqlite::Connection`, `SCHEMA_SQL` (déjà existant).
- Produces: `open_and_init(db_path: &Path) -> Result<Connection>` (signature inchangée), colonne `notes.position INTEGER NOT NULL DEFAULT 0` garantie présente après appel, backfillée par ordre décroissant de `updated_at` (la note la plus récente reçoit `position = 0`, la suivante `1`, etc.) pour préserver l'ordre d'affichage actuel au premier lancement après mise à jour.

- [ ] **Step 1: Write the failing test**

Ajouter en bas de `src/db.rs` :

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_SQL).unwrap();
        conn
    }

    #[test]
    fn migrate_adds_position_column_with_default_zero() {
        let mut conn = setup_conn();
        migrate_add_position_column(&mut conn).unwrap();

        let has_position: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name = 'position'")
            .unwrap()
            .query_row([], |row| row.get::<_, i64>(0))
            .unwrap()
            > 0;

        assert!(has_position, "expected notes.position column to exist after migration");
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut conn = setup_conn();
        migrate_add_position_column(&mut conn).unwrap();
        // Second call must not error (column already exists).
        migrate_add_position_column(&mut conn).unwrap();
    }

    #[test]
    fn migrate_backfills_position_by_recency() {
        let mut conn = setup_conn();
        insert_note(&mut conn, "oldest", &[]).unwrap();
        insert_note(&mut conn, "middle", &[]).unwrap();
        insert_note(&mut conn, "newest", &[]).unwrap();

        // Force distinct updated_at ordering regardless of insertion speed.
        conn.execute("UPDATE notes SET updated_at = '100' WHERE content = 'oldest'", [])
            .unwrap();
        conn.execute("UPDATE notes SET updated_at = '200' WHERE content = 'middle'", [])
            .unwrap();
        conn.execute("UPDATE notes SET updated_at = '300' WHERE content = 'newest'", [])
            .unwrap();

        migrate_add_position_column(&mut conn).unwrap();

        let mut stmt = conn
            .prepare("SELECT content FROM notes ORDER BY position ASC")
            .unwrap();
        let ordered: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert_eq!(ordered, vec!["newest", "middle", "oldest"]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test migrate_ -- --test-threads=1`
Expected: FAIL with "cannot find function `migrate_add_position_column`"

- [ ] **Step 3: Write minimal implementation**

Dans `src/db.rs`, ajouter avant `open_and_init` :

```rust
fn migrate_add_position_column(conn: &mut Connection) -> Result<()> {
    let has_position: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name = 'position'")
        .context("failed to inspect notes table schema")?
        .query_row([], |row| row.get::<_, i64>(0))
        .context("failed to check for position column")?
        > 0;

    if has_position {
        return Ok(());
    }

    let tx = conn
        .transaction()
        .context("failed to start position migration transaction")?;

    tx.execute("ALTER TABLE notes ADD COLUMN position INTEGER NOT NULL DEFAULT 0", [])
        .context("failed to add position column")?;

    tx.execute(
        "UPDATE notes SET position = (
            SELECT COUNT(*) FROM notes AS n2
            WHERE n2.updated_at > notes.updated_at
        )",
        [],
    )
    .context("failed to backfill position by recency")?;

    tx.commit().context("failed to commit position migration")?;
    Ok(())
}
```

Puis modifier `open_and_init` :

```rust
pub fn open_and_init(db_path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(db_path).context("failed to open sqlite database")?;
    conn.execute_batch(SCHEMA_SQL)
        .context("failed to initialize sqlite schema")?;
    migrate_add_position_column(&mut conn)?;
    Ok(conn)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test migrate_ -- --test-threads=1`
Expected: PASS (3 tests: `migrate_adds_position_column_with_default_zero`, `migrate_is_idempotent`, `migrate_backfills_position_by_recency`)

- [ ] **Step 5: Commit**

```bash
git add src/db.rs
git commit -m "feat: add notes.position column with recency-based backfill migration"
```

---

### Task 2: `insert_note` assigne une position en tête de liste

**Files:**
- Modify: `src/db.rs` (fonction `insert_note`)
- Test: `src/db.rs` (module `tests`)

**Interfaces:**
- Consumes: `migrate_add_position_column` (Task 1, garantit la colonne existe avant tout appel à `insert_note` en usage réel via `open_and_init`).
- Produces: `insert_note(conn: &mut Connection, content: &str, tags: &[String]) -> Result<()>` (signature inchangée) — après insertion, la nouvelle note a `position` strictement inférieure à toutes les positions existantes (elle apparaît en premier dans le tri manuel `ORDER BY position ASC`).

- [ ] **Step 1: Write the failing test**

Ajouter dans `mod tests` :

```rust
#[test]
fn insert_note_gets_lowest_position() {
    let mut conn = setup_conn();
    migrate_add_position_column(&mut conn).unwrap();

    insert_note(&mut conn, "first", &[]).unwrap();
    insert_note(&mut conn, "second", &[]).unwrap();

    let mut stmt = conn
        .prepare("SELECT content FROM notes ORDER BY position ASC")
        .unwrap();
    let ordered: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    assert_eq!(ordered, vec!["second", "first"]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test insert_note_gets_lowest_position -- --test-threads=1`
Expected: FAIL (both notes have `position = 0`, order is unspecified/insertion order, assertion fails since "first" won't sort after "second" reliably — the test asserts the new expected behavior which isn't implemented yet)

- [ ] **Step 3: Write minimal implementation**

Dans `src/db.rs`, modifier `insert_note` pour calculer la position minimale actuelle moins un avant d'insérer :

```rust
pub fn insert_note(conn: &mut Connection, content: &str, tags: &[String]) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    let now = now_unix_seconds()?;

    let tx = conn
        .transaction()
        .context("failed to start note insertion transaction")?;

    let min_position: i64 = tx
        .query_row("SELECT COALESCE(MIN(position), 0) FROM notes", [], |row| row.get(0))
        .context("failed to read current minimum note position")?;
    let new_position = min_position - 1;

    tx.execute(
        "INSERT INTO notes (id, content, created_at, updated_at, pinned, position) VALUES (?1, ?2, ?3, ?4, 0, ?5)",
        params![id, content, now, now, new_position],
    )
    .context("failed to insert note")?;

    tx.execute(
        "INSERT INTO notes_fts (note_id, content) VALUES (?1, ?2)",
        params![id, content],
    )
    .context("failed to index note in FTS table")?;

    for tag in &normalize_tags(tags) {
        tx.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            params![tag],
        )
        .context("failed to upsert tag")?;

        tx.execute(
            "INSERT INTO notes_tags (note_id, tag_id)
             SELECT ?1, id FROM tags WHERE name = ?2",
            params![id, tag],
        )
        .context("failed to link tag to note")?;
    }

    tx.commit()
        .context("failed to commit note insertion transaction")?;

    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test insert_note_gets_lowest_position -- --test-threads=1`
Expected: PASS

Run also: `cargo test -- --test-threads=1` (full suite, ensure Task 1 tests still pass)
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add src/db.rs
git commit -m "feat: new notes get the lowest position so they appear first in manual order"
```

---

### Task 3: `move_note` — déplacement d'une position dans l'ordre manuel

**Files:**
- Modify: `src/db.rs` (nouvelle fonction publique)
- Test: `src/db.rs` (module `tests`)

**Interfaces:**
- Consumes: connexion migrée (Task 1), notes déjà insérées (Task 2).
- Produces: `pub enum MoveDirection { Up, Down }` et `pub fn move_note(conn: &mut Connection, note_id: &str, direction: MoveDirection) -> Result<()>`. `Up` échange la position de `note_id` avec celle de la note immédiatement précédente dans l'ordre `position ASC` (position plus faible) ; `Down` avec la note immédiatement suivante (position plus élevée). Sans voisin dans la direction demandée, la fonction ne fait rien (pas d'erreur).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn move_note_up_swaps_with_previous() {
    let mut conn = setup_conn();
    migrate_add_position_column(&mut conn).unwrap();
    insert_note(&mut conn, "a", &[]).unwrap();
    insert_note(&mut conn, "b", &[]).unwrap();
    insert_note(&mut conn, "c", &[]).unwrap();
    // Manual order is now: c, b, a (most recently inserted first, see Task 2).

    let ids: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, content FROM notes ORDER BY position ASC")
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    let a_id = ids.iter().find(|(_, c)| c == "a").unwrap().0.clone();

    // "a" is last; moving it up should swap it with "b".
    move_note(&mut conn, &a_id, MoveDirection::Up).unwrap();

    let mut stmt = conn
        .prepare("SELECT content FROM notes ORDER BY position ASC")
        .unwrap();
    let ordered: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    assert_eq!(ordered, vec!["c", "a", "b"]);
}

#[test]
fn move_note_up_at_top_is_noop() {
    let mut conn = setup_conn();
    migrate_add_position_column(&mut conn).unwrap();
    insert_note(&mut conn, "only", &[]).unwrap();

    let id: String = conn
        .query_row("SELECT id FROM notes", [], |row| row.get(0))
        .unwrap();

    move_note(&mut conn, &id, MoveDirection::Up).unwrap();

    let position: i64 = conn
        .query_row("SELECT position FROM notes WHERE id = ?1", params![id], |row| row.get(0))
        .unwrap();
    assert_eq!(position, -1); // unchanged from insert_note's assignment
}

#[test]
fn move_note_down_swaps_with_next() {
    let mut conn = setup_conn();
    migrate_add_position_column(&mut conn).unwrap();
    insert_note(&mut conn, "a", &[]).unwrap();
    insert_note(&mut conn, "b", &[]).unwrap();
    // Manual order is now: b, a.

    let b_id: String = conn
        .query_row("SELECT id FROM notes WHERE content = 'b'", [], |row| row.get(0))
        .unwrap();

    move_note(&mut conn, &b_id, MoveDirection::Down).unwrap();

    let mut stmt = conn
        .prepare("SELECT content FROM notes ORDER BY position ASC")
        .unwrap();
    let ordered: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    assert_eq!(ordered, vec!["a", "b"]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test move_note_ -- --test-threads=1`
Expected: FAIL with "cannot find function `move_note`" / "cannot find type `MoveDirection`"

- [ ] **Step 3: Write minimal implementation**

Ajouter dans `src/db.rs` :

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Up,
    Down,
}

pub fn move_note(conn: &mut Connection, note_id: &str, direction: MoveDirection) -> Result<()> {
    let tx = conn
        .transaction()
        .context("failed to start move_note transaction")?;

    let current_position: i64 = tx
        .query_row(
            "SELECT position FROM notes WHERE id = ?1 AND deleted_at IS NULL",
            params![note_id],
            |row| row.get(0),
        )
        .context("failed to read current note position")?;

    let neighbor: Option<(String, i64)> = match direction {
        MoveDirection::Up => tx
            .query_row(
                "SELECT id, position FROM notes
                 WHERE deleted_at IS NULL AND position < ?1
                 ORDER BY position DESC LIMIT 1",
                params![current_position],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .context("failed to find previous neighbor")?,
        MoveDirection::Down => tx
            .query_row(
                "SELECT id, position FROM notes
                 WHERE deleted_at IS NULL AND position > ?1
                 ORDER BY position ASC LIMIT 1",
                params![current_position],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .context("failed to find next neighbor")?,
    };

    let Some((neighbor_id, neighbor_position)) = neighbor else {
        return Ok(());
    };

    tx.execute(
        "UPDATE notes SET position = ?2 WHERE id = ?1",
        params![note_id, neighbor_position],
    )
    .context("failed to update moved note position")?;

    tx.execute(
        "UPDATE notes SET position = ?2 WHERE id = ?1",
        params![neighbor_id, current_position],
    )
    .context("failed to update neighbor note position")?;

    tx.commit().context("failed to commit move_note transaction")?;
    Ok(())
}
```

Ajouter `rusqlite::OptionalExtension` à l'import en tête de fichier :

```rust
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -- --test-threads=1` (full suite)
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add src/db.rs
git commit -m "feat: add move_note for manual note reordering"
```

---

### Task 4: Tri conditionnel par position manuelle dans `search_notes`

**Files:**
- Modify: `src/db.rs` (fonction `search_notes`)
- Test: `src/db.rs` (module `tests`)

**Interfaces:**
- Consumes: `move_note`, `insert_note` (Tasks 2-3).
- Produces: `search_notes` (signature inchangée) trie désormais par `position ASC` quand `query` est vide ET `tags` est vide ; conserve le tri actuel (pertinence FTS5 ou `updated_at DESC`) dans tous les autres cas.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn search_notes_uses_manual_position_when_unfiltered() {
    let mut conn = setup_conn();
    migrate_add_position_column(&mut conn).unwrap();
    insert_note(&mut conn, "a", &[]).unwrap();
    insert_note(&mut conn, "b", &[]).unwrap();
    let b_id: String = conn
        .query_row("SELECT id FROM notes WHERE content = 'b'", [], |row| row.get(0))
        .unwrap();
    // Manual order is b, a. Move b down so order becomes a, b.
    move_note(&mut conn, &b_id, MoveDirection::Down).unwrap();

    let results = search_notes(&conn, "", &[], 10).unwrap();
    let contents: Vec<String> = results.into_iter().map(|n| n.preview).collect();

    assert_eq!(contents, vec!["a", "b"]);
}

#[test]
fn search_notes_ignores_manual_position_when_query_present() {
    let mut conn = setup_conn();
    migrate_add_position_column(&mut conn).unwrap();
    insert_note(&mut conn, "alpha", &[]).unwrap();
    insert_note(&mut conn, "beta", &[]).unwrap();

    // With a text query, ordering falls back to bm25/updated_at, not position.
    // We only assert that both matches are returned (behavior unchanged),
    // not a specific order, since that path is untouched by this task.
    let results = search_notes(&conn, "alpha OR beta", &[], 10).unwrap();
    assert_eq!(results.len(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test search_notes_uses_manual_position -- --test-threads=1`
Expected: FAIL (current `ORDER BY n.updated_at DESC` gives `b, a` not `a, b`)

- [ ] **Step 3: Write minimal implementation**

Dans `src/db.rs`, fonction `search_notes`, remplacer le bloc de tri :

```rust
    if query.is_empty() {
        sql.push_str("ORDER BY n.updated_at DESC ");
    } else {
        sql.push_str("ORDER BY bm25(notes_fts), n.updated_at DESC ");
    }
```

par :

```rust
    if query.is_empty() && normalized_tags.is_empty() {
        sql.push_str("ORDER BY n.position ASC ");
    } else if query.is_empty() {
        sql.push_str("ORDER BY n.updated_at DESC ");
    } else {
        sql.push_str("ORDER BY bm25(notes_fts), n.updated_at DESC ");
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -- --test-threads=1` (full suite)
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add src/db.rs
git commit -m "feat: sort notes by manual position when no search or tag filter is active"
```

---

### Task 5: HeaderBar remplace PopoverMenuBar + switcher-wrap

**Files:**
- Modify: `src/app.rs:359-395` (construction de `menu_bar`, `switcher_wrap`, ajout à `root`)
- Modify: `src/app.rs` (CSS dans `install_css`, classes `.menu-bar`/`.switcher-wrap` remplacées)

**Interfaces:**
- Consumes: `stack` (déjà construit), `app` (pour l'action quit).
- Produces: une `gtk::HeaderBar` assignée via `window.set_titlebar(Some(&header_bar))`, contenant le `StackSwitcher` en `title_widget` et un bouton quitter en `pack_end`. Aucune fonction publique nouvelle ; changement localisé à la construction de fenêtre.

- [ ] **Step 1: Vérifier l'état actuel (pas de test automatisé possible sur du GTK layout)**

Run: `cargo build 2>&1 | tail -5`
Expected: build actuel réussi (baseline avant modification)

- [ ] **Step 2: Remplacer la construction menu + switcher par une HeaderBar**

Dans `src/app.rs`, supprimer entièrement ce bloc (lignes actuelles de `let app_menu = gio::Menu::new();` jusqu'à `menu_bar.add_css_class("menu-bar");` inclus, ainsi que la définition de `switcher_wrap` juste avant) :

```rust
        let stack_switcher = StackSwitcher::new();
        stack_switcher.set_stack(Some(&stack));
        stack_switcher.set_halign(Align::Center);

        let switcher_wrap = GtkBox::new(Orientation::Horizontal, 8);
        switcher_wrap.add_css_class("switcher-wrap");
        switcher_wrap.append(&stack_switcher);

        let app_menu = gio::Menu::new();

        let item_capture = gio::MenuItem::new(Some("Capture"), Some("app.show_capture"));
        item_capture.set_attribute_value("icon", Some(&"document-edit-symbolic".to_variant()));
        app_menu.append_item(&item_capture);

        let item_notes = gio::MenuItem::new(Some("Notes"), Some("app.show_notes"));
        item_notes.set_attribute_value("icon", Some(&"view-list-symbolic".to_variant()));
        app_menu.append_item(&item_notes);

        let item_quit = gio::MenuItem::new(Some("Quitter"), Some("app.quit"));
        item_quit.set_attribute_value("icon", Some(&"application-exit-symbolic".to_variant()));
        app_menu.append_item(&item_quit);

        let menu_root = gio::Menu::new();
        menu_root.append_submenu(Some("Memo-Tori"), &app_menu);

        let menu_bar = PopoverMenuBar::from_model(Some(&menu_root));
        menu_bar.add_css_class("menu-bar");
```

Remplacer par :

```rust
        let stack_switcher = StackSwitcher::new();
        stack_switcher.set_stack(Some(&stack));
        stack_switcher.set_halign(Align::Center);

        let quit_btn = Button::from_icon_name("application-exit-symbolic");
        quit_btn.set_tooltip_text(Some("Quitter l'application"));
        quit_btn.update_property(&[gtk::accessible::Property::Label("Quitter l'application")]);

        let header_bar = gtk::HeaderBar::new();
        header_bar.set_title_widget(Some(&stack_switcher));
        header_bar.pack_end(&quit_btn);

        quit_btn.connect_clicked({
            let app = app.clone();
            move |_| app.quit()
        });
```

Puis dans la construction des actions (`action_show_capture`, `action_show_notes`, `action_quit`), retirer uniquement les accélérateurs devenus inutiles ? Non : garder `app.set_accels_for_action` tel quel (Ctrl+1/2/Q restent des raccourcis utiles indépendamment du menu visuel), mais retirer `action_show_capture`/`action_show_notes`/`action_quit` seulement s'ils ne sont plus référencés — ils restent nécessaires pour les accélérateurs clavier, donc **garder ces trois blocs `gio::SimpleAction` inchangés**.

Retirer l'import `PopoverMenuBar` désormais inutilisé et l'import `ToVariant` si plus utilisé ailleurs (vérifier avant de retirer) :

```rust
use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, ListBox,
    ListBoxRow, Orientation, Paned, Popover, ScrolledWindow, SearchEntry, Stack,
    StackSwitcher, TextView, WrapMode,
};
```

(retirer `PopoverMenuBar` de la liste ; garder `gio` car `gio::SimpleAction` est toujours utilisé).

Modifier l'assemblage de `root` :

```rust
        root.append(&menu_bar);
        root.append(&switcher_wrap);
        root.append(&stack);
        window.set_child(Some(&root));
```

devient :

```rust
        root.append(&stack);
        window.set_child(Some(&root));
        window.set_titlebar(Some(&header_bar));
```

- [ ] **Step 3: Mettre à jour le CSS**

Dans `install_css()`, remplacer :

```css
.menu-bar {
  background: #1f3a45;
  color: #f8fafc;
  padding: 6px;
  border-radius: 8px;
}

.switcher-wrap {
  background: #d8dfdc;
  padding: 6px;
  border-radius: 10px;
}
```

par :

```css
headerbar {
  background: #1f3a45;
  color: #f8fafc;
}

headerbar stackswitcher button {
  color: #dfe9ec;
}

headerbar stackswitcher button:checked {
  background: #f8fafc;
  color: #1f3a45;
}
```

- [ ] **Step 4: Vérifier la compilation puis lancer l'app manuellement**

Run: `cargo build 2>&1 | tail -30`
Expected: build sans erreur ni warning

Lancer l'app (`HOME=<home réel> nohup ./target/debug/memo-tori-gtk > /tmp/.../log 2>&1 &`), utiliser `xdotool search --name "Memo-Tori"` pour trouver l'ID de fenêtre réel (pas de fenêtre Firefox homonyme), puis `import -window <id> capture.png`, puis `Read` l'image pour vérifier visuellement :
- La HeaderBar affiche le switcher Capture/Notes centré, sur fond bleu-pétrole.
- Le bouton quitter est visible en bout de barre à droite.
- Basculer entre Capture et Notes fonctionne toujours (clic sur le switcher).

Supprimer la capture après vérification si elle contient des données réelles de l'utilisateur (notes personnelles visibles dans la liste), ne jamais la conserver dans le repo ni la renvoyer sans nécessité.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: replace PopoverMenuBar and custom switcher wrap with a native HeaderBar"
```

---

### Task 6: Fusionner les actions de sauvegarde du panneau Notes

**Files:**
- Modify: `src/app.rs` (construction de `edit_tags_row`, handlers `apply_tags_btn`/`save_note_btn`)

**Interfaces:**
- Consumes: `db::replace_note_tags`, `db::update_note_content` (existants, inchangés), `notes_state`, `list_box`, `reader`, `selected_tags_entry`.
- Produces: un unique bouton "Enregistrer" appelant les deux opérations DB en séquence pour la note sélectionnée, plus un bouton "Annuler" qui recharge contenu + tags depuis la DB (réutilise la logique déjà présente dans `list_box.connect_row_selected`).

- [ ] **Step 1: Baseline**

Run: `cargo build 2>&1 | tail -5`
Expected: build actuel réussi

- [ ] **Step 2: Extraire la logique de rechargement en closure réutilisable**

Dans `src/app.rs`, avant la définition de `list_box.connect_row_selected`, extraire son corps en une closure nommée `load_selected_note` réutilisable par le futur bouton Annuler :

```rust
        let load_selected_note: Rc<dyn Fn()> = {
            let conn = Rc::clone(&conn);
            let reader = reader.clone();
            let notes_state = Rc::clone(&notes_state);
            let selected_tags_label = selected_tags_label.clone();
            let selected_tags_entry = selected_tags_entry.clone();
            let list_box = list_box.clone();

            Rc::new(move || {
                let Some(row) = list_box.selected_row() else {
                    reader.buffer().set_text("Aucune note sélectionnée.");
                    selected_tags_label.set_text("Tags : -");
                    selected_tags_entry.set_text("");
                    return;
                };

                let index = row.index();
                if index < 0 {
                    reader.buffer().set_text("Aucune note sélectionnée.");
                    selected_tags_label.set_text("Tags : -");
                    selected_tags_entry.set_text("");
                    return;
                }

                let note_id = notes_state
                    .borrow()
                    .get(index as usize)
                    .map(|note| note.id.clone());

                let Some(note_id) = note_id else {
                    reader.buffer().set_text("Aucune note sélectionnée.");
                    selected_tags_label.set_text("Tags : -");
                    selected_tags_entry.set_text("");
                    return;
                };

                match db::get_note_content(&conn.borrow(), &note_id) {
                    Ok(Some(content)) => reader.buffer().set_text(&content),
                    Ok(None) => reader.buffer().set_text("Note introuvable."),
                    Err(err) => reader
                        .buffer()
                        .set_text(&format!("Échec du chargement de la note :\n{}", err)),
                }

                match db::get_note_tags(&conn.borrow(), &note_id) {
                    Ok(tags) => {
                        if tags.is_empty() {
                            selected_tags_label.set_text("Tags : -");
                            selected_tags_entry.set_text("");
                        } else {
                            let joined = tags.join(", ");
                            selected_tags_label.set_text(&format!("Tags : {}", joined));
                            selected_tags_entry.set_text(&joined);
                        }
                    }
                    Err(_) => {
                        selected_tags_label.set_text("Tags : erreur de chargement");
                    }
                }
            })
        };

        list_box.connect_row_selected({
            let load_selected_note = Rc::clone(&load_selected_note);
            move |_, _| load_selected_note.as_ref()()
        });
```

Ceci remplace entièrement l'ancien bloc `list_box.connect_row_selected({ ... })` (qui contenait cette logique inline).

- [ ] **Step 3: Remplacer les deux boutons par Enregistrer/Annuler**

Remplacer la construction de `edit_tags_row` :

```rust
        let edit_tags_row = GtkBox::new(Orientation::Horizontal, 8);
        let selected_tags_entry = Entry::new();
        selected_tags_entry.set_hexpand(true);
        selected_tags_entry.set_placeholder_text(Some("Tags de la note selectionnee"));
        attach_tag_autocomplete(&selected_tags_entry, Rc::clone(&conn));
        let apply_tags_btn = icon_label_button("emblem-ok-symbolic", "Apply tags");
        apply_tags_btn.set_tooltip_text(Some("Appliquer les tags a la note selectionnee"));
        let save_note_btn = icon_label_button("document-save-symbolic", "Save note");
        save_note_btn.set_tooltip_text(Some("Sauvegarder les modifications de la note"));
        edit_tags_row.append(&selected_tags_entry);
        edit_tags_row.append(&apply_tags_btn);
        edit_tags_row.append(&save_note_btn);
```

par :

```rust
        let selected_tags_label_field = Label::new(Some("Tags de la note sélectionnée"));
        selected_tags_label_field.set_halign(Align::Start);
        selected_tags_label_field.add_css_class("field-label");

        let selected_tags_entry = Entry::new();
        selected_tags_entry.set_hexpand(true);
        selected_tags_entry.set_placeholder_text(Some("ex : projet, idée"));
        attach_tag_autocomplete(&selected_tags_entry, Rc::clone(&conn));
        selected_tags_label_field.set_mnemonic_widget(Some(&selected_tags_entry));

        let note_actions_row = GtkBox::new(Orientation::Horizontal, 8);
        note_actions_row.set_halign(Align::End);
        let cancel_note_btn = icon_label_button("edit-undo-symbolic", "Annuler");
        cancel_note_btn.set_tooltip_text(Some("Annuler les modifications non enregistrées"));
        let save_note_btn = icon_label_button("document-save-symbolic", "Enregistrer");
        save_note_btn.set_tooltip_text(Some("Enregistrer le contenu et les tags de cette note"));
        note_actions_row.append(&cancel_note_btn);
        note_actions_row.append(&save_note_btn);
```

Adapter l'assemblage (remplacer la ligne `library_panel.append(&edit_tags_row);` par les nouveaux widgets, dans l'ordre : label de tags, entry de tags, puis plus loin après le `paned`, la rangée d'actions) :

```rust
        library_panel.append(&search_row);
        library_panel.append(&selected_tags_label_field);
        library_panel.append(&selected_tags_entry);
        library_panel.append(&selected_tags_label);
        library_panel.append(&paned);
        library_panel.append(&note_actions_row);
```

- [ ] **Step 4: Remplacer les handlers `apply_tags_btn`/`save_note_btn` par les nouveaux handlers fusionnés**

Supprimer entièrement l'ancien bloc `apply_tags_btn.connect_clicked({ ... })` et l'ancien bloc `save_note_btn.connect_clicked({ ... })`, remplacer par :

```rust
        save_note_btn.connect_clicked({
            let conn = Rc::clone(&conn);
            let notes_state = Rc::clone(&notes_state);
            let list_box = list_box.clone();
            let reader = reader.clone();
            let selected_tags_entry = selected_tags_entry.clone();
            let refresh_notes = Rc::clone(&refresh_notes);
            move |_| {
                let Some(row) = list_box.selected_row() else {
                    return;
                };

                let index = row.index();
                if index < 0 {
                    return;
                }

                let note_id = notes_state
                    .borrow()
                    .get(index as usize)
                    .map(|n| n.id.clone());

                let Some(note_id) = note_id else {
                    return;
                };

                let buffer = reader.buffer();
                let start = buffer.start_iter();
                let end = buffer.end_iter();
                let content = buffer.text(&start, &end, true).to_string();
                let tags = parse_tags(&selected_tags_entry.text());

                let content_result = db::update_note_content(&mut conn.borrow_mut(), &note_id, content.trim());
                let tags_result = db::replace_note_tags(&mut conn.borrow_mut(), &note_id, &tags);

                if content_result.is_ok() && tags_result.is_ok() {
                    let _ = Notification::new()
                        .summary("Memo-Tori")
                        .body("Note enregistrée")
                        .show();
                    refresh_notes.as_ref()();
                }
            }
        });

        cancel_note_btn.connect_clicked({
            let load_selected_note = Rc::clone(&load_selected_note);
            move |_| load_selected_note.as_ref()()
        });
```

- [ ] **Step 5: Build, lancer et vérifier manuellement**

Run: `cargo build 2>&1 | tail -30`
Expected: build sans erreur ni warning (attention : `apply_tags_btn` n'existe plus, vérifier qu'aucune autre référence n'y pointe)

Lancer l'app, sélectionner une note dans l'onglet Notes, modifier le texte et les tags, cliquer "Enregistrer" : vérifier que les deux changements persistent après un `refresh_notes`. Cliquer "Annuler" après une modification non enregistrée : vérifier que le contenu et les tags reviennent à l'état stocké.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat: merge tag and content save actions into a single Enregistrer button"
```

---

### Task 7: Passage intégral des textes UI en français

**Files:**
- Modify: `src/app.rs` (tous les literals de texte visibles listés dans la spec)

**Interfaces:**
- Consumes: aucun changement de signature.
- Produces: aucun texte anglais résiduel visible par l'utilisateur.

- [ ] **Step 1: Baseline**

Run: `grep -nE '"(Save|Cancel|Search notes|No notes yet|No note selected|Note not found|Search error|Search failed|Failed to load note)"' src/app.rs`
Expected: liste des occurrences à remplacer (baseline avant modification)

- [ ] **Step 2: Remplacer chaque occurrence**

Dans `src/app.rs`, appliquer la table de correspondance :

- `let save_btn = icon_label_button("document-save-symbolic", "Save");` → `"Enregistrer"`
- `let cancel_btn = icon_label_button("edit-clear-symbolic", "Cancel");` → `"Annuler"`
- `search_entry.set_placeholder_text(Some("Search notes (FTS5)"));` → `Some("Rechercher...")`
- `reader.buffer().set_text("No notes yet.");` → `"Aucune note pour l'instant."`
- `reader.buffer().set_text("No note selected.");` (toutes occurrences, y compris celles déjà réécrites en français dans `load_selected_note` à la Task 6 — vérifier cohérence) → `"Aucune note sélectionnée."`
- `status_label.set_text("Search error");` → `"Échec de la recherche"`
- `reader.buffer().set_text(&format!("Search failed:\n{}", err));` → `&format!("Échec de la recherche :\n{}", err)`
- `status_label` initial `Label::new(Some("0 notes"))` → `Label::new(Some("0 note"))`
- `status_label.set_text(&format!("{} notes", notes.len()));` → gérer le pluriel : `&format!("{} note{}", notes.len(), if notes.len() > 1 { "s" } else { "" })`

- [ ] **Step 3: Vérifier qu'aucun texte anglais ne subsiste**

Run: `grep -nE '"(Save|Cancel|Search notes|No notes yet|No note selected|Note not found|Search error|Search failed|Failed to load note)"' src/app.rs`
Expected: aucune sortie (0 correspondance)

Run: `cargo build 2>&1 | tail -20`
Expected: build sans erreur ni warning

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: translate remaining UI strings to French"
```

---

### Task 8: Labels visibles sur les champs de saisie

**Files:**
- Modify: `src/app.rs` (constructions de `search_entry`, `filter_tags_entry`, `capture_tags`, `text_view`)

**Interfaces:**
- Consumes: aucun changement de signature de fonction.
- Produces: chaque champ de saisie principal a un `Label` visible immédiatement au-dessus, lié via `set_mnemonic_widget`.

- [ ] **Step 1: Baseline**

Run: `cargo build 2>&1 | tail -5`

- [ ] **Step 2: Ajouter les labels**

Pour `search_entry` (dans `search_row`, avant sa création) :

```rust
        let search_label = Label::new(Some("Rechercher"));
        search_label.set_halign(Align::Start);
        search_label.add_css_class("field-label");
```

Restructurer `search_row` en `GtkBox` verticale contenant `search_label` puis une ligne horizontale avec les champs, ou plus simplement ajouter les labels dans une ligne dédiée au-dessus de `search_row` :

```rust
        let search_labels_row = GtkBox::new(Orientation::Horizontal, 8);
        let filter_label = Label::new(Some("Filtrer par tags"));
        filter_label.set_halign(Align::Start);
        filter_label.add_css_class("field-label");
        search_label.set_hexpand(true);
        filter_label.set_hexpand(true);
        search_labels_row.append(&search_label);
        search_labels_row.append(&filter_label);
        search_label.set_mnemonic_widget(Some(&search_entry));
        filter_label.set_mnemonic_widget(Some(&filter_tags_entry));
```

Ajouter `library_panel.append(&search_labels_row);` juste avant `library_panel.append(&search_row);`.

Pour `capture_tags` (dans le panneau Capture) :

```rust
        let capture_tags_label = Label::new(Some("Tags"));
        capture_tags_label.set_halign(Align::Start);
        capture_tags_label.add_css_class("field-label");
        capture_tags_label.set_mnemonic_widget(Some(&capture_tags));
```

Ajouter `capture_panel.append(&capture_tags_label);` juste avant `capture_panel.append(&capture_tags);`.

Pour `text_view` (zone de capture elle-même) :

```rust
        let capture_field_label = Label::new(Some("Nouvelle note"));
        capture_field_label.set_halign(Align::Start);
        capture_field_label.add_css_class("field-label");
```

Ajouter `capture_panel.append(&capture_field_label);` juste avant `capture_panel.append(&capture_overlay);` (remplace en partie le rôle de `capture_label` existant "Capture d'idee rapide" — garder les deux : `capture_label` reste le titre de section, `capture_field_label` devient le label du champ texte lui-même). Renommer aussi `capture_label` (titre) en français correct : `"Capture d'idée rapide"`.

- [ ] **Step 3: Ajouter la classe CSS `.field-label`**

Dans `install_css()`, ajouter :

```css
.field-label {
  color: #1f3a45;
  font-weight: 600;
  font-size: 0.85rem;
  margin-bottom: 2px;
}
```

- [ ] **Step 4: Build, lancer et vérifier visuellement**

Run: `cargo build 2>&1 | tail -30`
Expected: build sans erreur

Lancer l'app, capturer la fenêtre (méthode Task 5 Step 4), vérifier que chaque champ a bien un label visible juste au-dessus, sans casser la mise en page existante.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: add visible labels above all input fields for accessibility"
```

---

### Task 9: Bannière d'erreur visible et annoncée

**Files:**
- Modify: `src/app.rs` (nouveau widget `error_banner`, branché sur les échecs `db::insert_note`, `db::update_note_content`, `db::replace_note_tags`)

**Interfaces:**
- Consumes: résultats `Result<()>` déjà retournés par les fonctions `db::*` existantes (aucun changement dans `src/db.rs`).
- Produces: closure `show_error: Rc<dyn Fn(&str)>` affichant le message dans `error_banner` et le rendant visible ; les points d'appel `on_save` (capture) et `save_note_btn` (Task 6) l'utilisent en cas d'échec au lieu d'ignorer silencieusement l'erreur.

- [ ] **Step 1: Baseline**

Run: `cargo build 2>&1 | tail -5`

- [ ] **Step 2: Créer le widget et la closure d'affichage**

Juste après la création de `root` (avant `stack`), ajouter :

```rust
        let error_banner = Label::new(None);
        error_banner.set_halign(Align::Start);
        error_banner.add_css_class("error-banner");
        error_banner.set_visible(false);
        error_banner.set_accessible_role(gtk::AccessibleRole::Alert);
```

Ajouter `root.append(&error_banner);` juste avant `root.append(&stack);`.

Créer la closure de notification d'erreur (après la définition de `refresh_notes`, avant `on_save`) :

```rust
        let show_error: Rc<dyn Fn(&str)> = {
            let error_banner = error_banner.clone();
            Rc::new(move |message: &str| {
                error_banner.set_text(message);
                error_banner.set_visible(true);
            })
        };
```

- [ ] **Step 3: Brancher sur les points d'échec existants**

Dans `on_save` (capture), remplacer :

```rust
                if db::insert_note(&mut conn.borrow_mut(), trimmed, &tags).is_ok() {
                    buffer.set_text("");
                    capture_tags.set_text("");
                    let _ = Notification::new()
                        .summary("Memo-Tori")
                        .body("Note saved")
                        .show();
                    refresh_notes.as_ref()();
                }
```

par :

```rust
                match db::insert_note(&mut conn.borrow_mut(), trimmed, &tags) {
                    Ok(()) => {
                        buffer.set_text("");
                        capture_tags.set_text("");
                        error_banner.set_visible(false);
                        let _ = Notification::new()
                            .summary("Memo-Tori")
                            .body("Note enregistrée")
                            .show();
                        refresh_notes.as_ref()();
                    }
                    Err(err) => show_error(&format!("Échec de l'enregistrement : {}", err)),
                }
```

(nécessite de capturer `error_banner` et `show_error` dans la closure `on_save` — ajouter `let error_banner = error_banner.clone();` et `let show_error = Rc::clone(&show_error);` dans son bloc de capture).

Dans le handler `save_note_btn` (Task 6), remplacer la fin par :

```rust
                let content_result = db::update_note_content(&mut conn.borrow_mut(), &note_id, content.trim());
                let tags_result = db::replace_note_tags(&mut conn.borrow_mut(), &note_id, &tags);

                match (content_result, tags_result) {
                    (Ok(()), Ok(())) => {
                        error_banner.set_visible(false);
                        let _ = Notification::new()
                            .summary("Memo-Tori")
                            .body("Note enregistrée")
                            .show();
                        refresh_notes.as_ref()();
                    }
                    (Err(err), _) | (_, Err(err)) => {
                        show_error(&format!("Échec de l'enregistrement : {}", err));
                    }
                }
```

(ajouter `let error_banner = error_banner.clone();` et `let show_error = Rc::clone(&show_error);` dans les captures de ce handler).

- [ ] **Step 4: CSS de la bannière (vérifier contraste avant intégration)**

Calculer le ratio avant d'ajouter (rouge sombre `#7a1f1f` sur fond clair `#fbe4e4`) :

Run: `python3 -c "
def hex_to_rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i+2], 16) for i in (0, 2, 4))
def luminance(rgb):
    def chan(c):
        c = c / 255.0
        return c/12.92 if c <= 0.03928 else ((c+0.055)/1.055) ** 2.4
    r, g, b = rgb
    return 0.2126*chan(r) + 0.7152*chan(g) + 0.0722*chan(b)
def contrast(hex1, hex2):
    l1, l2 = luminance(hex_to_rgb(hex1)), luminance(hex_to_rgb(hex2))
    lighter, darker = max(l1, l2), min(l1, l2)
    return (lighter + 0.05) / (darker + 0.05)
print(contrast('#7a1f1f', '#fbe4e4'))
"`
Expected: ratio >= 4.5 (si inférieur, assombrir le texte ou éclaircir le fond jusqu'à passer AA, puis reporter les valeurs finales dans le CSS ci-dessous)

Ajouter dans `install_css()` :

```css
.error-banner {
  background: #fbe4e4;
  color: #7a1f1f;
  font-weight: 600;
  padding: 8px 12px;
  border-radius: 8px;
  border: 1px solid #d98c8c;
}
```

- [ ] **Step 5: Build, lancer et vérifier**

Run: `cargo build 2>&1 | tail -30`
Expected: build sans erreur

Test manuel du chemin d'erreur difficile à déclencher normalement (DB en lecture seule) : `chmod 444 ~/.local/share/memo-tori/memo-tori.db`, lancer l'app, tenter de sauvegarder une note, vérifier que la bannière rouge apparaît avec un message clair au lieu d'un échec silencieux. Remettre les permissions ensuite : `chmod 644 ~/.local/share/memo-tori/memo-tori.db`.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat: show visible, screen-reader-announced errors on save failures"
```

---

### Task 10: Brancher `config.text_scale`

**Files:**
- Modify: `src/app.rs` (signature de `install_css`, appel dans `connect_activate`)

**Interfaces:**
- Consumes: `config.text_scale: f32` (déjà existant dans `AppConfig`, jamais lu jusqu'ici).
- Produces: `install_css(text_scale: f32)` — le CSS généré applique une taille de police de base multipliée par `text_scale`.

- [ ] **Step 1: Baseline**

Run: `grep -n "text_scale" src/app.rs src/config.rs`
Expected: présent seulement dans `config.rs`, absent de `app.rs` (confirme le point mort actuel)

- [ ] **Step 2: Modifier `install_css` pour accepter et appliquer le facteur**

Changer la signature :

```rust
fn install_css() {
```

en :

```rust
fn install_css(text_scale: f32) {
```

Dans le bloc CSS, la règle :

```css
* {
  font-family: "Nebula Sans", "Inter", "Noto Sans", "DejaVu Sans", sans-serif;
}
```

devient générée dynamiquement (remplacer `load_from_data` par une construction de chaîne via `format!`) :

```rust
    let base_font_size = (13.0 * text_scale.max(0.5).min(3.0)).round() as i32;
    let css = format!(
        "
window {{
  background: linear-gradient(180deg, #f4f0e6 0%, #ece7dc 100%);
}}

* {{
  font-family: \"Nebula Sans\", \"Inter\", \"Noto Sans\", \"DejaVu Sans\", sans-serif;
  font-size: {base_font_size}px;
}}
",
    );
```

(fusionner cette portion générée avec le reste du CSS statique existant : soit tout convertir en un seul `format!` avec des accolades doublées `{{`/`}}` pour échapper le CSS littéral, soit charger le CSS statique restant via un second appel à `provider.load_from_data` — préférer le second, plus simple, en gardant le gros bloc CSS existant tel quel dans une constante séparée et en ajoutant un `provider` supplémentaire juste pour la règle de taille de police, avec une priorité CSS légèrement supérieure pour qu'elle prenne le dessus si conflit).

Implémentation retenue (deux providers) :

```rust
fn install_css(text_scale: f32) {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(BASE_CSS);

    let scale_provider = gtk::CssProvider::new();
    let base_font_size = (13.0 * text_scale.clamp(0.5, 3.0)).round() as i32;
    scale_provider.load_from_data(&format!("* {{ font-size: {base_font_size}px; }}"));

    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        gtk::style_context_add_provider_for_display(
            &display,
            &scale_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
    }
}
```

Extraire tout le contenu CSS actuellement inline dans `load_from_data("...")` vers une constante `const BASE_CSS: &str = "...";` au-dessus de `install_css` (même contenu, juste déplacé), en retirant la règle `font-family`+`font-size` combinée pour ne garder que `font-family` dans `BASE_CSS` :

```css
* {
  font-family: "Nebula Sans", "Inter", "Noto Sans", "DejaVu Sans", sans-serif;
}
```

- [ ] **Step 3: Mettre à jour l'appel**

Remplacer `install_css();` par `install_css(config.text_scale);` — nécessite que `config` (le paramètre de `run`) soit accessible à cet endroit. Vérifier qu'une copie de `text_scale` est capturée avant le déplacement de `config.capture_hints` (déjà fait via `config.capture_hints.clone()` plus haut) : ajouter juste après `let capture_hints = ...;` :

```rust
    let text_scale = config.text_scale;
```

Puis dans la closure `connect_activate` (qui capture déjà `main_window`), ajouter `text_scale` à ses captures :

```rust
    app.connect_activate({
        let main_window = Rc::clone(&main_window);
        move |app| {
        if let Some((window, _stack)) = main_window.borrow().as_ref() {
            window.present();
            return;
        }

        gtk::Window::set_default_icon_name("memo-tori");
        crate::fonts::ensure_installed();
        install_css(text_scale);
```

(`text_scale` est un `f32`, `Copy`, donc capturé par valeur sans souci de `Rc`/`RefCell`).

- [ ] **Step 4: Build et vérifier visuellement à deux échelles**

Run: `cargo build 2>&1 | tail -30`
Expected: build sans erreur

Éditer temporairement `~/.config/memo-tori/config.toml` pour mettre `text_scale = 1.5`, lancer l'app, capturer la fenêtre, vérifier que le texte est visiblement plus grand qu'à `1.0`. Remettre `text_scale = 1.0` ensuite.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: apply config.text_scale to the UI font size"
```

---

### Task 11: Aide raccourcis visible

**Files:**
- Modify: `src/app.rs` (panneau Capture)

**Interfaces:**
- Consumes: aucun changement de signature.
- Produces: un `Label` discret sous la zone de capture listant les raccourcis actifs.

- [ ] **Step 1: Baseline**

Run: `cargo build 2>&1 | tail -5`

- [ ] **Step 2: Ajouter le label d'aide**

Après la construction de `capture_tags` et avant `actions`, ajouter :

```rust
        let shortcuts_hint = Label::new(Some(
            "Entrée : enregistrer · Échap : effacer · Ctrl+Tab : changer d'onglet",
        ));
        shortcuts_hint.set_halign(Align::Start);
        shortcuts_hint.add_css_class("shortcuts-hint");
```

Ajouter `capture_panel.append(&shortcuts_hint);` juste avant `capture_panel.append(&actions);`.

- [ ] **Step 3: CSS**

Dans `BASE_CSS` (constante extraite à la Task 10), ajouter :

```css
.shortcuts-hint {
  color: #496067;
  font-size: 0.8rem;
  font-style: italic;
}
```

- [ ] **Step 4: Build et vérifier visuellement**

Run: `cargo build 2>&1 | tail -30`
Expected: build sans erreur

Lancer l'app, capturer la fenêtre, vérifier que le texte d'aide est visible sous la zone de capture, discret mais lisible.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: show a visible keyboard shortcuts hint in the capture panel"
```

---

### Task 12: Glisser-déposer des notes (souris)

**Files:**
- Modify: `src/app.rs` (construction de `list_box`, boucle de remplissage dans `refresh_notes`)

**Interfaces:**
- Consumes: `db::move_note`, `db::MoveDirection` (Task 3), `notes_state`, `refresh_notes`.
- Produces: chaque `ListBoxRow` de la liste de notes est source de drag (`gtk::DragSource`) portant l'ID de la note ; la `ListBox` est cible de drop (`gtk::DropTarget`) qui déplace la note déposée à la position de la ligne cible via des appels successifs à `db::move_note`. Le drag est désactivé (DragSource non ajouté) quand une recherche ou un filtre tag est actif.

- [ ] **Step 1: Baseline**

Run: `cargo build 2>&1 | tail -5`

- [ ] **Step 2: Ajouter une fonction `manual_order_active` et le comportement drag**

Dans `src/app.rs`, ajouter une fonction utilitaire près de `parse_tags` :

```rust
fn manual_order_active(search_entry: &SearchEntry, filter_tags_entry: &Entry) -> bool {
    search_entry.text().trim().is_empty() && filter_tags_entry.text().trim().is_empty()
}
```

Dans la boucle de remplissage de `refresh_notes` (`for item in &notes { ... }`), après la création de `row` et avant `list_box.append(&row)`, ajouter le drag source conditionnel :

```rust
                            if manual_order_active(&search_entry, &filter_tags_entry) {
                                let drag_source = gtk::DragSource::new();
                                let note_id = item.id.clone();
                                drag_source.connect_prepare(move |_, _, _| {
                                    Some(gtk::gdk::ContentProvider::for_value(&note_id.to_value()))
                                });
                                row.add_controller(drag_source);
                            }
```

`refresh_notes` n'existe pas encore au moment où `list_box` est construite (il est défini plus loin dans la fonction), donc le drop handler ne peut pas le capturer directement. Déclarer un conteneur différé **avant** la construction de `list_box`, en portée de fonction :

```rust
        let refresh_notes_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
```

Ajouter le drop target sur `list_box`, une seule fois, juste après sa création (`let list_box = ListBox::new();`) :

```rust
        list_box.set_selection_mode(gtk::SelectionMode::Single);

        let drop_target = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
        drop_target.connect_drop({
            let conn = Rc::clone(&conn);
            let notes_state = Rc::clone(&notes_state);
            let refresh_notes_holder = Rc::clone(&refresh_notes_holder);
            move |target, value, _x, y| {
                let Ok(dragged_id) = value.get::<String>() else {
                    return false;
                };

                let Some(target_row) = target.widget().downcast_ref::<ListBox>()
                    .and_then(|lb| lb.row_at_y(y as i32))
                else {
                    return false;
                };

                let target_index = target_row.index();
                if target_index < 0 {
                    return false;
                }

                let target_id = notes_state
                    .borrow()
                    .get(target_index as usize)
                    .map(|n| n.id.clone());

                let Some(target_id) = target_id else {
                    return false;
                };

                if dragged_id == target_id {
                    return false;
                }

                let dragged_position = notes_state
                    .borrow()
                    .iter()
                    .position(|n| n.id == dragged_id);
                let target_position = notes_state
                    .borrow()
                    .iter()
                    .position(|n| n.id == target_id);

                let (Some(dragged_position), Some(target_position)) = (dragged_position, target_position) else {
                    return false;
                };

                let direction = if dragged_position > target_position {
                    db::MoveDirection::Up
                } else {
                    db::MoveDirection::Down
                };

                let mut current = dragged_position;
                while current != target_position {
                    let id_at_current = notes_state.borrow().get(current).map(|n| n.id.clone());
                    let Some(id_at_current) = id_at_current else { break };
                    if db::move_note(&mut conn.borrow_mut(), &id_at_current, direction).is_err() {
                        break;
                    }
                    current = match direction {
                        db::MoveDirection::Up => current.saturating_sub(1),
                        db::MoveDirection::Down => current + 1,
                    };
                }

                if let Some(refresh) = refresh_notes_holder.borrow().as_ref() {
                    refresh.as_ref()();
                }

                true
            }
        });
        list_box.add_controller(drop_target);
```

Juste après la définition de `refresh_notes` (bloc `let refresh_notes: Rc<dyn Fn()> = { ... };`), remplir le conteneur différé pour que le drop handler ci-dessus puisse l'appeler :

```rust
        *refresh_notes_holder.borrow_mut() = Some(Rc::clone(&refresh_notes));
```

- [ ] **Step 3: Build et vérifier manuellement le drag & drop**

Run: `cargo build 2>&1 | tail -30`
Expected: build sans erreur (ajuster les imports GTK si `gdk::ContentProvider` ou `ToValue` manquent — ajouter `use gtk::glib::value::ToValue;` et vérifier `gtk::gdk` déjà importé)

Lancer l'app, aller dans l'onglet Notes (sans recherche ni filtre actif), faire glisser une ligne de note vers une autre position dans la liste avec la souris (`xdotool` peut simuler un drag : `xdotool mousedown`, `mousemove`, `mouseup` sur les coordonnées des lignes capturées via une image intermédiaire, ou test manuel direct si une session interactive est disponible). Vérifier que l'ordre persiste après un redémarrage de l'app (relire `~/.local/share/memo-tori/memo-tori.db` ou simplement fermer/rouvrir l'app et comparer l'ordre affiché).

Vérifier aussi que taper une recherche désactive visuellement le drag (les nouvelles lignes affichées pendant une recherche ne doivent plus déclencher de `DragSource` — vérifiable en tentant un drag pendant une recherche active et en confirmant qu'aucun déplacement n'a lieu).

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: add drag-and-drop manual reordering of notes"
```

---

### Task 13: Équivalent clavier du réordonnancement (WCAG 2.1.1)

**Files:**
- Modify: `src/app.rs` (nouveau `EventControllerKey` sur `list_box`)

**Interfaces:**
- Consumes: `db::move_note`, `db::MoveDirection` (Task 3), `manual_order_active` (Task 12).
- Produces: `Alt+Flèche haut` déplace la note sélectionnée vers le haut dans l'ordre manuel, `Alt+Flèche bas` vers le bas, uniquement quand `manual_order_active` est vrai ; sans effet sinon.

- [ ] **Step 1: Baseline**

Run: `cargo build 2>&1 | tail -5`

- [ ] **Step 2: Ajouter le contrôleur clavier**

Après le bloc `list_box.connect_row_selected({ ... });` (Task 6), ajouter :

```rust
        let reorder_key_controller = gtk::EventControllerKey::new();
        reorder_key_controller.connect_key_pressed({
            let conn = Rc::clone(&conn);
            let notes_state = Rc::clone(&notes_state);
            let list_box = list_box.clone();
            let search_entry = search_entry.clone();
            let filter_tags_entry = filter_tags_entry.clone();
            let refresh_notes = Rc::clone(&refresh_notes);
            move |_, key, _, state| {
                if !state.contains(gdk::ModifierType::ALT_MASK) {
                    return Propagation::Proceed;
                }

                if !manual_order_active(&search_entry, &filter_tags_entry) {
                    return Propagation::Proceed;
                }

                let direction = if key == gdk::Key::Up {
                    Some(db::MoveDirection::Up)
                } else if key == gdk::Key::Down {
                    Some(db::MoveDirection::Down)
                } else {
                    None
                };

                let Some(direction) = direction else {
                    return Propagation::Proceed;
                };

                let Some(row) = list_box.selected_row() else {
                    return Propagation::Proceed;
                };

                let index = row.index();
                if index < 0 {
                    return Propagation::Proceed;
                }

                let note_id = notes_state
                    .borrow()
                    .get(index as usize)
                    .map(|n| n.id.clone());

                let Some(note_id) = note_id else {
                    return Propagation::Proceed;
                };

                if db::move_note(&mut conn.borrow_mut(), &note_id, direction).is_ok() {
                    refresh_notes.as_ref()();
                }

                Propagation::Stop
            }
        });
        list_box.add_controller(reorder_key_controller);
```

- [ ] **Step 3: Build et vérifier manuellement**

Run: `cargo build 2>&1 | tail -30`
Expected: build sans erreur

Lancer l'app, onglet Notes, sélectionner une note à la souris (pour établir la sélection initiale — la navigation clavier standard de `ListBox` avec les flèches seules fonctionne déjà nativement pour changer la sélection), puis utiliser `Alt+Flèche haut`/`Alt+Flèche bas` pour la déplacer dans la liste. Vérifier que ça ne fonctionne plus dès qu'une recherche est tapée dans `search_entry`.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: add Alt+Up/Down keyboard equivalent for note reordering"
```

---

### Task 14: Vérification finale complète

**Files:** aucun fichier modifié, vérification uniquement.

- [ ] **Step 1: Suite de tests complète**

Run: `cargo test -- --test-threads=1`
Expected: tous les tests PASS (Tasks 1-4)

- [ ] **Step 2: Build sans warning**

Run: `cargo build 2>&1 | grep -E "warning|error"`
Expected: aucune sortie

- [ ] **Step 3: Grep résiduel de textes anglais**

Run: `grep -nE '"\s*(Save|Cancel|Apply tags|Save note)\s*"' src/app.rs`
Expected: aucune sortie

- [ ] **Step 4: Vérification manuelle bout-en-bout**

Lancer l'app réelle, dérouler le parcours complet : capturer une note avec tags → basculer sur Notes via la HeaderBar → rechercher → effacer la recherche → réordonner deux notes à la souris → réordonner au clavier (Alt+Haut/Bas) → modifier contenu+tags d'une note et Enregistrer → cliquer Annuler après une modification non sauvegardée → déclencher une erreur (DB en lecture seule) et vérifier la bannière. Capturer la fenêtre à 2-3 étapes clés pour confirmation visuelle, sans jamais capturer le bureau entier ni conserver de capture contenant des notes personnelles réelles de l'utilisateur au-delà de la vérification immédiate.

- [ ] **Step 5: Commit final si des ajustements ont eu lieu pendant la vérification**

```bash
git add -A
git commit -m "fix: address issues found during end-to-end UI/UX verification"
```

(ne committer que s'il y a effectivement eu des changements ; sinon, cette tâche se conclut sans commit.)
