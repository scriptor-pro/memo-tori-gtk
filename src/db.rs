use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection};
use uuid::Uuid;

const SCHEMA_SQL: &str = include_str!("../migrations/001_init.sql");

#[derive(Debug, Clone)]
pub struct NoteListItem {
    pub id: String,
    pub preview: String,
}

pub fn open_and_init(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path).context("failed to open sqlite database")?;
    conn.execute_batch(SCHEMA_SQL)
        .context("failed to initialize sqlite schema")?;
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

    tx.execute(
        "INSERT INTO notes (id, content, created_at, updated_at, pinned) VALUES (?1, ?2, ?3, ?4, 0)",
        params![id, content, now, now],
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
