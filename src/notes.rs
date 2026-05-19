use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::cli::SourceSelector;
use crate::localdb::open_connection;
use crate::sources::scope_label_for_notes;

#[derive(Clone, Debug, Serialize)]
pub struct NoteSummary {
    pub id: i64,
    pub scope: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NoteRevision {
    pub version: i64,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

pub fn add_note(
    selector: &SourceSelector,
    title: Option<&str>,
    body: &str,
    tags: &[String],
) -> Result<NoteSummary> {
    let scope = scope_label_for_notes(selector)?;
    let conn = open_connection()?;
    let now = Utc::now().to_rfc3339();
    let title = title
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| derive_title(body));
    conn.execute(
        "INSERT INTO notes (scope, title, body, tags_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![scope, title, body, serde_json::to_string(tags)?, now, now],
    )?;
    let note_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO note_revisions (note_id, version, title, body, tags_json, created_at)
         VALUES (?1, 1, ?2, ?3, ?4, ?5)",
        params![note_id, title, body, serde_json::to_string(tags)?, now],
    )?;
    get_note(note_id)?.ok_or_else(|| anyhow!("failed to reload created note"))
}

pub fn update_note(
    id: i64,
    title: Option<&str>,
    body: &str,
    tags: &[String],
) -> Result<NoteSummary> {
    let conn = open_connection()?;
    let existing = get_note(id)?.ok_or_else(|| anyhow!("note id {id} not found"))?;
    let next_title = title
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or(existing.title.clone());
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE notes SET title = ?1, body = ?2, tags_json = ?3, updated_at = ?4 WHERE id = ?5",
        params![next_title, body, serde_json::to_string(tags)?, now, id],
    )?;
    let version = revision_count(id)? + 1;
    conn.execute(
        "INSERT INTO note_revisions (note_id, version, title, body, tags_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            version,
            next_title,
            body,
            serde_json::to_string(tags)?,
            now
        ],
    )?;
    get_note(id)?.ok_or_else(|| anyhow!("failed to reload updated note"))
}

pub fn list_notes(
    selector: Option<&SourceSelector>,
    query: Option<&str>,
) -> Result<Vec<NoteSummary>> {
    let conn = open_connection()?;
    let mut stmt = conn.prepare(
        "SELECT id, scope, title, body, tags_json, created_at, updated_at
         FROM notes
         ORDER BY updated_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let tags_json: String = row.get(4)?;
        Ok(NoteSummary {
            id: row.get(0)?,
            scope: row.get(1)?,
            title: row.get(2)?,
            body: row.get(3)?,
            tags: serde_json::from_str(&tags_json).unwrap_or_default(),
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;

    let scope_filter = selector
        .map(scope_label_for_notes)
        .transpose()?
        .map(|scope| scope.to_ascii_lowercase());
    let query_filter = query.map(|value| value.to_ascii_lowercase());

    let mut out = Vec::new();
    for row in rows {
        let note = row?;
        if let Some(scope) = scope_filter.as_deref() {
            if note.scope.to_ascii_lowercase() != scope {
                continue;
            }
        }
        if let Some(query) = query_filter.as_deref() {
            let haystack = format!(
                "{} {} {} {}",
                note.scope,
                note.title,
                note.body,
                note.tags.join(" ")
            )
            .to_ascii_lowercase();
            if !haystack.contains(query) {
                continue;
            }
        }
        out.push(note);
    }
    Ok(out)
}

pub fn note_history(id: i64) -> Result<Vec<NoteRevision>> {
    let conn = open_connection()?;
    let mut stmt = conn.prepare(
        "SELECT version, title, body, tags_json, created_at
         FROM note_revisions
         WHERE note_id = ?1
         ORDER BY version DESC",
    )?;
    let rows = stmt.query_map([id], |row| {
        let tags_json: String = row.get(3)?;
        Ok(NoteRevision {
            version: row.get(0)?,
            title: row.get(1)?,
            body: row.get(2)?,
            tags: serde_json::from_str(&tags_json).unwrap_or_default(),
            created_at: row.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn render_notes(notes: &[NoteSummary]) -> String {
    if notes.is_empty() {
        return "No notes found.\n".to_string();
    }
    let mut out = String::new();
    for note in notes {
        out.push_str(&format!(
            "#{} [{}] {}\nTags: {}\n{}\nUpdated: {}\n\n",
            note.id,
            note.scope,
            note.title,
            if note.tags.is_empty() {
                "-".to_string()
            } else {
                note.tags.join(", ")
            },
            note.body,
            note.updated_at
        ));
    }
    out
}

pub fn render_history(history: &[NoteRevision]) -> String {
    if history.is_empty() {
        return "No revision history.\n".to_string();
    }
    let mut out = String::new();
    for rev in history {
        out.push_str(&format!(
            "v{} [{}]\nTitle: {}\nTags: {}\n{}\n\n",
            rev.version,
            rev.created_at,
            rev.title,
            if rev.tags.is_empty() {
                "-".to_string()
            } else {
                rev.tags.join(", ")
            },
            rev.body
        ));
    }
    out
}

fn get_note(id: i64) -> Result<Option<NoteSummary>> {
    let conn = open_connection()?;
    conn.query_row(
        "SELECT id, scope, title, body, tags_json, created_at, updated_at
         FROM notes WHERE id = ?1",
        [id],
        |row| {
            let tags_json: String = row.get(4)?;
            Ok(NoteSummary {
                id: row.get(0)?,
                scope: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )
    .optional()
    .context("failed to load note")
}

fn revision_count(id: i64) -> Result<i64> {
    let conn = open_connection()?;
    let count = conn
        .query_row(
            "SELECT COUNT(*) FROM note_revisions WHERE note_id = ?1",
            [id],
            |row| row.get(0),
        )
        .context("failed to count note revisions")?;
    Ok(count)
}

fn derive_title(body: &str) -> String {
    body.lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.chars().count() > 48 {
                let mut short = trimmed.chars().take(48).collect::<String>();
                short.push('…');
                short
            } else {
                trimmed.to_string()
            }
        })
        .unwrap_or_else(|| "Untitled note".to_string())
}
