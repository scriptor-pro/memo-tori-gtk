use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use uuid::Uuid;

const SCHEMA_SQL: &str = include_str!("../migrations/001_init.sql");

#[derive(Debug, Clone)]
pub struct NoteListItem {
    pub id: String,
    pub preview: String,
}

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

pub fn open_and_init(db_path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(db_path).context("failed to open sqlite database")?;
    conn.execute_batch(SCHEMA_SQL)
        .context("failed to initialize sqlite schema")?;
    migrate_add_position_column(&mut conn)?;
    Ok(conn)
}

fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();

    for tag in tags {
        let clean = tag.trim().to_lowercase();
        if clean.is_empty() {
            continue;
        }
        if !normalized.iter().any(|seen| seen == &clean) {
            normalized.push(clean);
        }
    }

    normalized
}

fn now_unix_seconds() -> Result<String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_secs()
        .to_string())
}

pub fn replace_note_tags(conn: &mut Connection, note_id: &str, tags: &[String]) -> Result<()> {
    let normalized = normalize_tags(tags);
    let tx = conn
        .transaction()
        .context("failed to start tags transaction")?;

    tx.execute(
        "DELETE FROM notes_tags WHERE note_id = ?1",
        params![note_id],
    )
    .context("failed to clear existing note tags")?;

    for tag in &normalized {
        tx.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            params![tag],
        )
        .context("failed to upsert tag")?;

        tx.execute(
            "INSERT INTO notes_tags (note_id, tag_id)
             SELECT ?1, id FROM tags WHERE name = ?2",
            params![note_id, tag],
        )
        .context("failed to link tag to note")?;
    }

    tx.commit().context("failed to commit tags transaction")?;
    Ok(())
}

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

pub fn update_note_content(conn: &mut Connection, note_id: &str, content: &str) -> Result<()> {
    let now = now_unix_seconds()?;
    let tx = conn
        .transaction()
        .context("failed to start note update transaction")?;

    tx.execute(
        "UPDATE notes SET content = ?2, updated_at = ?3
         WHERE id = ?1 AND deleted_at IS NULL",
        params![note_id, content, now],
    )
    .context("failed to update note")?;

    tx.execute("DELETE FROM notes_fts WHERE note_id = ?1", params![note_id])
        .context("failed to clear note FTS row")?;

    tx.execute(
        "INSERT INTO notes_fts (note_id, content) VALUES (?1, ?2)",
        params![note_id, content],
    )
    .context("failed to refresh note FTS row")?;

    tx.commit()
        .context("failed to commit note update transaction")?;

    Ok(())
}

pub fn search_notes(
    conn: &Connection,
    query: &str,
    tags: &[String],
    limit: i64,
) -> Result<Vec<NoteListItem>> {
    let query = query.trim();
    let normalized_tags = normalize_tags(tags);
    let mut args: Vec<Value> = Vec::new();
    let mut sql = String::from("SELECT n.id, n.content FROM notes n ");

    if !query.is_empty() {
        sql.push_str("JOIN notes_fts ON notes_fts.note_id = n.id ");
    }

    if !normalized_tags.is_empty() {
        sql.push_str(
            "JOIN notes_tags nt ON nt.note_id = n.id
             JOIN tags t ON t.id = nt.tag_id ",
        );
    }

    sql.push_str("WHERE n.deleted_at IS NULL ");

    if !query.is_empty() {
        sql.push_str("AND notes_fts MATCH ? ");
        args.push(Value::Text(query.to_string()));
    }

    if !normalized_tags.is_empty() {
        sql.push_str("AND t.name IN (");
        for (index, tag) in normalized_tags.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push('?');
            args.push(Value::Text(tag.clone()));
        }
        sql.push_str(") ");
    }

    if !normalized_tags.is_empty() {
        sql.push_str("GROUP BY n.id ");
        sql.push_str("HAVING COUNT(DISTINCT t.name) = ? ");
        args.push(Value::Integer(normalized_tags.len() as i64));
    }

    if query.is_empty() {
        sql.push_str("ORDER BY n.updated_at DESC ");
    } else {
        sql.push_str("ORDER BY bm25(notes_fts), n.updated_at DESC ");
    }

    sql.push_str("LIMIT ?");
    args.push(Value::Integer(limit));

    let mut stmt = conn
        .prepare(&sql)
        .context("failed to prepare note search query")?;

    let rows = stmt
        .query_map(params_from_iter(args.iter()), |row| {
            Ok(NoteListItem {
                id: row.get(0)?,
                preview: row.get(1)?,
            })
        })
        .context("failed to execute note search query")?;

    let items = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to decode note search results")?;

    Ok(items)
}

pub fn get_note_content(conn: &Connection, note_id: &str) -> Result<Option<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT content
             FROM notes
             WHERE id = ?1 AND deleted_at IS NULL",
        )
        .context("failed to prepare note lookup")?;

    let mut rows = stmt
        .query(params![note_id])
        .context("failed to execute note lookup")?;

    if let Some(row) = rows.next().context("failed to fetch note row")? {
        let content: String = row.get(0).context("failed to decode note content")?;
        return Ok(Some(content));
    }

    Ok(None)
}

pub fn get_note_tags(conn: &Connection, note_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT t.name
             FROM notes_tags nt
             JOIN tags t ON t.id = nt.tag_id
             WHERE nt.note_id = ?1
             ORDER BY t.name ASC",
        )
        .context("failed to prepare note tags query")?;

    let rows = stmt
        .query_map(params![note_id], |row| row.get::<_, String>(0))
        .context("failed to execute note tags query")?;

    let tags = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to decode note tags")?;

    Ok(tags)
}

pub fn list_tags_prefix(conn: &Connection, prefix: &str, limit: i64) -> Result<Vec<String>> {
    let prefix = prefix.trim().to_lowercase();
    if prefix.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare(
            "SELECT name
             FROM tags
             WHERE name LIKE ?1 || '%'
             ORDER BY name ASC
             LIMIT ?2",
        )
        .context("failed to prepare tag prefix query")?;

    let rows = stmt
        .query_map(params![prefix, limit], |row| row.get::<_, String>(0))
        .context("failed to execute tag prefix query")?;

    let tags = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to decode tag prefix results")?;

    Ok(tags)
}

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
        // Insert directly via SQL (bypassing `insert_note`) to reproduce notes
        // created before the `position` column existed, since `insert_note`
        // now requires that column to be present (Task 2).
        conn.execute(
            "INSERT INTO notes (id, content, created_at, updated_at, pinned) VALUES ('1', 'oldest', '100', '100', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO notes (id, content, created_at, updated_at, pinned) VALUES ('2', 'middle', '200', '200', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO notes (id, content, created_at, updated_at, pinned) VALUES ('3', 'newest', '300', '300', 0)",
            [],
        )
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
}
