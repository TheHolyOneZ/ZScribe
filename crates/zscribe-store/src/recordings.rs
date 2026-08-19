use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use ts_rs::TS;
use zscribe_core::{ActionItem, Segment, Summary, TokenUsage, Transcript};

const SCHEMA_VERSION: i32 = 6;

const SEARCH_SCHEMA: &str = r#"
    CREATE VIRTUAL TABLE search USING fts5 (
        recording_id UNINDEXED,
        title,
        body,
        summary,
        tokenize = 'unicode61 remove_diacritics 2'
    );

    CREATE TRIGGER search_after_recording_insert AFTER INSERT ON recordings BEGIN
        DELETE FROM search WHERE recording_id = new.id;
        INSERT INTO search (recording_id, title, body, summary)
        VALUES (
            new.id,
            new.title,
            COALESCE((SELECT full_text FROM transcripts WHERE recording_id = new.id), ''),
            COALESCE((SELECT body_md   FROM summaries   WHERE recording_id = new.id), '')
        );
    END;

    CREATE TRIGGER search_after_recording_rename AFTER UPDATE OF title ON recordings BEGIN
        DELETE FROM search WHERE recording_id = new.id;
        INSERT INTO search (recording_id, title, body, summary)
        VALUES (
            new.id,
            new.title,
            COALESCE((SELECT full_text FROM transcripts WHERE recording_id = new.id), ''),
            COALESCE((SELECT body_md   FROM summaries   WHERE recording_id = new.id), '')
        );
    END;

    CREATE TRIGGER search_after_recording_delete AFTER DELETE ON recordings BEGIN
        DELETE FROM search WHERE recording_id = old.id;
    END;

    CREATE TRIGGER search_after_transcript_insert AFTER INSERT ON transcripts BEGIN
        DELETE FROM search WHERE recording_id = new.recording_id;
        INSERT INTO search (recording_id, title, body, summary)
        SELECT r.id, r.title, new.full_text,
               COALESCE((SELECT body_md FROM summaries WHERE recording_id = r.id), '')
        FROM recordings r WHERE r.id = new.recording_id;
    END;

    CREATE TRIGGER search_after_transcript_update AFTER UPDATE ON transcripts BEGIN
        DELETE FROM search WHERE recording_id = new.recording_id;
        INSERT INTO search (recording_id, title, body, summary)
        SELECT r.id, r.title, new.full_text,
               COALESCE((SELECT body_md FROM summaries WHERE recording_id = r.id), '')
        FROM recordings r WHERE r.id = new.recording_id;
    END;

    CREATE TRIGGER search_after_summary_insert AFTER INSERT ON summaries BEGIN
        DELETE FROM search WHERE recording_id = new.recording_id;
        INSERT INTO search (recording_id, title, body, summary)
        SELECT r.id, r.title,
               COALESCE((SELECT full_text FROM transcripts WHERE recording_id = r.id), ''),
               new.body_md
        FROM recordings r WHERE r.id = new.recording_id;
    END;

    CREATE TRIGGER search_after_summary_update AFTER UPDATE ON summaries BEGIN
        DELETE FROM search WHERE recording_id = new.recording_id;
        INSERT INTO search (recording_id, title, body, summary)
        SELECT r.id, r.title,
               COALESCE((SELECT full_text FROM transcripts WHERE recording_id = r.id), ''),
               new.body_md
        FROM recordings r WHERE r.id = new.recording_id;
    END;

    -- Everything recorded before this migration. Without it, search would
    -- answer only for recordings made after the upgrade, which is the kind of
    -- half-working that is worse than not having it.
    INSERT INTO search (recording_id, title, body, summary)
    SELECT r.id, r.title,
           COALESCE((SELECT full_text FROM transcripts WHERE recording_id = r.id), ''),
           COALESCE((SELECT body_md   FROM summaries   WHERE recording_id = r.id), '')
    FROM recordings r;
"#;

const TAG_SCHEMA: &str = r#"
    CREATE TABLE tags (
        recording_id TEXT NOT NULL REFERENCES recordings (id) ON DELETE CASCADE,
        tag          TEXT NOT NULL,
        PRIMARY KEY (recording_id, tag)
    );

    CREATE INDEX tags_tag ON tags (tag);

    DROP TRIGGER search_after_recording_insert;
    DROP TRIGGER search_after_recording_rename;
    DROP TRIGGER search_after_recording_delete;
    DROP TRIGGER search_after_transcript_insert;
    DROP TRIGGER search_after_transcript_update;
    DROP TRIGGER search_after_summary_insert;
    DROP TRIGGER search_after_summary_update;
    DROP TABLE search;

    CREATE VIRTUAL TABLE search USING fts5 (
        recording_id UNINDEXED,
        title,
        body,
        summary,
        tags,
        tokenize = 'unicode61 remove_diacritics 2'
    );

    CREATE TRIGGER search_after_recording_insert AFTER INSERT ON recordings BEGIN
        DELETE FROM search WHERE recording_id = new.id;
        INSERT INTO search (recording_id, title, body, summary, tags)
        SELECT r.id, r.title,
               COALESCE((SELECT full_text FROM transcripts WHERE recording_id = r.id), ''),
               COALESCE((SELECT body_md   FROM summaries   WHERE recording_id = r.id), ''),
               COALESCE((SELECT group_concat(tag, ' ') FROM tags WHERE recording_id = r.id), '')
        FROM recordings r WHERE r.id = new.id;
    END;

    CREATE TRIGGER search_after_recording_rename AFTER UPDATE OF title ON recordings BEGIN
        DELETE FROM search WHERE recording_id = new.id;
        INSERT INTO search (recording_id, title, body, summary, tags)
        SELECT r.id, r.title,
               COALESCE((SELECT full_text FROM transcripts WHERE recording_id = r.id), ''),
               COALESCE((SELECT body_md   FROM summaries   WHERE recording_id = r.id), ''),
               COALESCE((SELECT group_concat(tag, ' ') FROM tags WHERE recording_id = r.id), '')
        FROM recordings r WHERE r.id = new.id;
    END;

    CREATE TRIGGER search_after_recording_delete AFTER DELETE ON recordings BEGIN
        DELETE FROM search WHERE recording_id = old.id;
    END;

    CREATE TRIGGER search_after_transcript_insert AFTER INSERT ON transcripts BEGIN
        DELETE FROM search WHERE recording_id = new.recording_id;
        INSERT INTO search (recording_id, title, body, summary, tags)
        SELECT r.id, r.title, new.full_text,
               COALESCE((SELECT body_md FROM summaries WHERE recording_id = r.id), ''),
               COALESCE((SELECT group_concat(tag, ' ') FROM tags WHERE recording_id = r.id), '')
        FROM recordings r WHERE r.id = new.recording_id;
    END;

    CREATE TRIGGER search_after_transcript_update AFTER UPDATE ON transcripts BEGIN
        DELETE FROM search WHERE recording_id = new.recording_id;
        INSERT INTO search (recording_id, title, body, summary, tags)
        SELECT r.id, r.title, new.full_text,
               COALESCE((SELECT body_md FROM summaries WHERE recording_id = r.id), ''),
               COALESCE((SELECT group_concat(tag, ' ') FROM tags WHERE recording_id = r.id), '')
        FROM recordings r WHERE r.id = new.recording_id;
    END;

    CREATE TRIGGER search_after_summary_insert AFTER INSERT ON summaries BEGIN
        DELETE FROM search WHERE recording_id = new.recording_id;
        INSERT INTO search (recording_id, title, body, summary, tags)
        SELECT r.id, r.title,
               COALESCE((SELECT full_text FROM transcripts WHERE recording_id = r.id), ''),
               new.body_md,
               COALESCE((SELECT group_concat(tag, ' ') FROM tags WHERE recording_id = r.id), '')
        FROM recordings r WHERE r.id = new.recording_id;
    END;

    CREATE TRIGGER search_after_summary_update AFTER UPDATE ON summaries BEGIN
        DELETE FROM search WHERE recording_id = new.recording_id;
        INSERT INTO search (recording_id, title, body, summary, tags)
        SELECT r.id, r.title,
               COALESCE((SELECT full_text FROM transcripts WHERE recording_id = r.id), ''),
               new.body_md,
               COALESCE((SELECT group_concat(tag, ' ') FROM tags WHERE recording_id = r.id), '')
        FROM recordings r WHERE r.id = new.recording_id;
    END;

    CREATE TRIGGER search_after_tag_insert AFTER INSERT ON tags BEGIN
        DELETE FROM search WHERE recording_id = new.recording_id;
        INSERT INTO search (recording_id, title, body, summary, tags)
        SELECT r.id, r.title,
               COALESCE((SELECT full_text FROM transcripts WHERE recording_id = r.id), ''),
               COALESCE((SELECT body_md   FROM summaries   WHERE recording_id = r.id), ''),
               COALESCE((SELECT group_concat(tag, ' ') FROM tags WHERE recording_id = r.id), '')
        FROM recordings r WHERE r.id = new.recording_id;
    END;

    CREATE TRIGGER search_after_tag_delete AFTER DELETE ON tags BEGIN
        DELETE FROM search WHERE recording_id = old.recording_id;
        INSERT INTO search (recording_id, title, body, summary, tags)
        SELECT r.id, r.title,
               COALESCE((SELECT full_text FROM transcripts WHERE recording_id = r.id), ''),
               COALESCE((SELECT body_md   FROM summaries   WHERE recording_id = r.id), ''),
               COALESCE((SELECT group_concat(tag, ' ') FROM tags WHERE recording_id = r.id), '')
        FROM recordings r WHERE r.id = old.recording_id;
    END;

    INSERT INTO search (recording_id, title, body, summary, tags)
    SELECT r.id, r.title,
           COALESCE((SELECT full_text FROM transcripts WHERE recording_id = r.id), ''),
           COALESCE((SELECT body_md   FROM summaries   WHERE recording_id = r.id), ''),
           ''
    FROM recordings r;
"#;

#[derive(Debug, Error)]
pub enum RecordingsError {
    #[error("recordings database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("stored transcript or summary is not valid JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error(
        "the recordings database was written by a newer version of ZScribe (schema {found}, this \
         build understands {SCHEMA_VERSION}); upgrade the app rather than downgrading it"
    )]
    FromTheFuture { found: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Recording {
    pub id: String,

    #[ts(type = "number")]
    pub started_at: i64,

    pub duration_ms: u32,
    pub source: String,
    pub template_id: String,
    pub title: String,

    pub audio_path: Option<String>,

    pub has_transcript: bool,
    pub has_summary: bool,

    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewRecording {
    pub id: String,
    pub started_at: i64,
    pub duration_ms: u32,
    pub source: String,
    pub template_id: String,
    pub title: String,
    pub audio_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecordingDetail {
    pub recording: Recording,
    pub transcript: Option<Transcript>,
    pub summary: Option<Summary>,
}

const PASSAGE_SCHEMA: &str = r#"
    CREATE TABLE passages (
        recording_id TEXT    NOT NULL REFERENCES recordings (id) ON DELETE CASCADE,
        ord          INTEGER NOT NULL,
        start_ms     INTEGER NOT NULL,
        end_ms       INTEGER NOT NULL,
        text         TEXT    NOT NULL,
        model        TEXT    NOT NULL,
        embedding    BLOB    NOT NULL,
        PRIMARY KEY (recording_id, ord)
    );

    CREATE INDEX passages_model ON passages (model);
"#;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PassageHit {
    pub recording_id: String,
    pub title: String,

    #[ts(type = "number")]
    pub started_at: i64,

    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,

    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchHit {
    pub recording: Recording,

    pub snippet: String,
}

type SummaryRow = (String, String, String, String, String, u32, u32, u32, u32);

pub struct Recordings {
    conn: Connection,
}

impl Recordings {
    pub fn open(path: &Path) -> Result<Self, RecordingsError> {
        let conn = Connection::open(path)?;
        Self::prepare(conn)
    }

    pub fn in_memory() -> Result<Self, RecordingsError> {
        Self::prepare(Connection::open_in_memory()?)
    }

    fn prepare(conn: Connection) -> Result<Self, RecordingsError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), RecordingsError> {
        let found: i32 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;

        if found > SCHEMA_VERSION {
            return Err(RecordingsError::FromTheFuture { found });
        }

        for version in found..SCHEMA_VERSION {
            match version {
                0 => self.conn.execute_batch(
                    "CREATE TABLE recordings (
                         id           TEXT PRIMARY KEY,
                         started_at   INTEGER NOT NULL,
                         duration_ms  INTEGER NOT NULL,
                         source       TEXT NOT NULL,
                         template_id  TEXT NOT NULL,
                         title        TEXT NOT NULL,
                         audio_path   TEXT
                     );

                     CREATE INDEX recordings_started_at ON recordings (started_at DESC);

                     CREATE TABLE transcripts (
                         recording_id TEXT PRIMARY KEY
                                      REFERENCES recordings (id) ON DELETE CASCADE,
                         language     TEXT NOT NULL,
                         model        TEXT NOT NULL,
                         full_text    TEXT NOT NULL,
                         segments     TEXT NOT NULL
                     );

                     CREATE TABLE summaries (
                         recording_id  TEXT PRIMARY KEY
                                       REFERENCES recordings (id) ON DELETE CASCADE,
                         provider      TEXT NOT NULL,
                         model         TEXT NOT NULL,
                         template_id   TEXT NOT NULL,
                         body_md       TEXT NOT NULL,
                         action_items  TEXT NOT NULL,
                         input_tokens  INTEGER NOT NULL,
                         output_tokens INTEGER NOT NULL,
                         elapsed_ms    INTEGER NOT NULL
                     );",
                )?,

                1 => self.conn.execute_batch(SEARCH_SCHEMA)?,

                2 => self.conn.execute_batch(PASSAGE_SCHEMA)?,

                3 => self.conn.execute_batch(TAG_SCHEMA)?,

                4 => self.conn.execute_batch(
                    "ALTER TABLE summaries ADD COLUMN redacted INTEGER NOT NULL DEFAULT 0;",
                )?,

                5 => self
                    .conn
                    .execute_batch("ALTER TABLE recordings ADD COLUMN note_path TEXT;")?,

                _ => unreachable!("no migration defined for schema version {version}"),
            }
        }

        self.conn
            .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(())
    }

    pub fn insert(&self, recording: &NewRecording) -> Result<(), RecordingsError> {
        self.conn.execute(
            "INSERT INTO recordings
                 (id, started_at, duration_ms, source, template_id, title, audio_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                recording.id,
                recording.started_at,
                recording.duration_ms,
                recording.source,
                recording.template_id,
                recording.title,
                recording.audio_path,
            ],
        )?;
        Ok(())
    }

    pub fn set_transcript(
        &self,
        recording_id: &str,
        transcript: &Transcript,
    ) -> Result<(), RecordingsError> {
        self.conn.execute(
            "INSERT INTO transcripts (recording_id, language, model, full_text, segments)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (recording_id) DO UPDATE SET
                 language = excluded.language,
                 model = excluded.model,
                 full_text = excluded.full_text,
                 segments = excluded.segments",
            params![
                recording_id,
                transcript.language,
                transcript.model,
                transcript.text(),
                serde_json::to_string(&transcript.segments)?,
            ],
        )?;

        self.conn.execute(
            "DELETE FROM passages WHERE recording_id = ?1",
            [recording_id],
        )?;

        Ok(())
    }

    pub fn set_summary(
        &self,
        recording_id: &str,
        summary: &Summary,
    ) -> Result<(), RecordingsError> {
        self.conn.execute(
            "INSERT INTO summaries (recording_id, provider, model, template_id, body_md,
                                    action_items, input_tokens, output_tokens, elapsed_ms,
                                    redacted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT (recording_id) DO UPDATE SET
                 provider = excluded.provider,
                 model = excluded.model,
                 template_id = excluded.template_id,
                 body_md = excluded.body_md,
                 action_items = excluded.action_items,
                 input_tokens = excluded.input_tokens,
                 output_tokens = excluded.output_tokens,
                 elapsed_ms = excluded.elapsed_ms,
                 redacted = excluded.redacted",
            params![
                recording_id,
                summary.provider,
                summary.model,
                summary.template_id,
                summary.body_md,
                serde_json::to_string(&summary.action_items)?,
                summary.usage.input,
                summary.usage.output,
                summary.elapsed_ms,
                summary.redacted,
            ],
        )?;
        Ok(())
    }

    pub fn set_title(&self, recording_id: &str, title: &str) -> Result<(), RecordingsError> {
        self.conn.execute(
            "UPDATE recordings SET title = ?2 WHERE id = ?1",
            params![recording_id, title],
        )?;
        Ok(())
    }

    pub fn set_duration(
        &self,
        recording_id: &str,
        duration_ms: u32,
    ) -> Result<(), RecordingsError> {
        self.conn.execute(
            "UPDATE recordings SET duration_ms = ?2 WHERE id = ?1",
            params![recording_id, duration_ms],
        )?;
        Ok(())
    }

    pub fn list(&self, limit: u32) -> Result<Vec<Recording>, RecordingsError> {
        let mut statement = self.conn.prepare(
            "SELECT r.id, r.started_at, r.duration_ms, r.source, r.template_id, r.title,
                    r.audio_path,
                    t.recording_id IS NOT NULL,
                    s.recording_id IS NOT NULL
             FROM recordings r
             LEFT JOIN transcripts t ON t.recording_id = r.id
             LEFT JOIN summaries   s ON s.recording_id = r.id
             ORDER BY r.started_at DESC
             LIMIT ?1",
        )?;

        let rows = statement.query_map([limit], row_to_recording)?;
        let mut recordings = rows.collect::<Result<Vec<_>, _>>()?;
        self.attach_tags(&mut recordings)?;
        Ok(recordings)
    }

    fn attach_tags(&self, recordings: &mut [Recording]) -> Result<(), RecordingsError> {
        if recordings.is_empty() {
            return Ok(());
        }

        let mut statement = self
            .conn
            .prepare("SELECT recording_id, tag FROM tags ORDER BY tag")?;

        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut by_recording: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for row in rows {
            let (id, tag) = row?;
            by_recording.entry(id).or_default().push(tag);
        }

        for recording in recordings {
            if let Some(tags) = by_recording.remove(&recording.id) {
                recording.tags = tags;
            }
        }

        Ok(())
    }

    pub fn set_tags(&self, recording_id: &str, tags: &[String]) -> Result<(), RecordingsError> {
        let mut wanted: Vec<String> = Vec::new();
        for tag in tags {
            let tag = tag.trim();
            if tag.is_empty() || tag.len() > 64 {
                continue;
            }
            if !wanted
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(tag))
            {
                wanted.push(tag.to_owned());
            }
        }

        let transaction = self.conn.unchecked_transaction()?;
        transaction.execute("DELETE FROM tags WHERE recording_id = ?1", [recording_id])?;

        {
            let mut statement =
                transaction.prepare("INSERT INTO tags (recording_id, tag) VALUES (?1, ?2)")?;
            for tag in &wanted {
                statement.execute(params![recording_id, tag])?;
            }
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn tags(&self) -> Result<Vec<(String, u32)>, RecordingsError> {
        let mut statement = self.conn.prepare(
            "SELECT tag, COUNT(*) AS uses FROM tags GROUP BY tag ORDER BY uses DESC, tag ASC",
        )?;

        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<SearchHit>, RecordingsError> {
        let Some(expression) = fts_query(query) else {
            return Ok(Vec::new());
        };

        let mut statement = self.conn.prepare(
            "SELECT r.id, r.started_at, r.duration_ms, r.source, r.template_id, r.title,
                    r.audio_path,
                    t.recording_id IS NOT NULL,
                    s.recording_id IS NOT NULL,
                    snippet(search, -1, ?3, ?4, '…', 12)
             FROM search
             JOIN recordings r  ON r.id = search.recording_id
             LEFT JOIN transcripts t ON t.recording_id = r.id
             LEFT JOIN summaries   s ON s.recording_id = r.id
             WHERE search MATCH ?1
             ORDER BY bm25(search, 0.0, 10.0, 1.0, 2.0, 12.0)
             LIMIT ?2",
        )?;

        let rows =
            statement.query_map(params![expression, limit, MATCH_OPEN, MATCH_CLOSE], |row| {
                Ok(SearchHit {
                    recording: row_to_recording(row)?,
                    snippet: row.get(9)?,
                })
            })?;

        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn set_passages(
        &self,
        recording_id: &str,
        model: &str,
        passages: &[(zscribe_core::Passage, Vec<f32>)],
    ) -> Result<(), RecordingsError> {
        let transaction = self.conn.unchecked_transaction()?;

        transaction.execute(
            "DELETE FROM passages WHERE recording_id = ?1",
            [recording_id],
        )?;

        {
            let mut statement = transaction.prepare(
                "INSERT INTO passages (recording_id, ord, start_ms, end_ms, text, model, embedding)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;

            for (ord, (passage, embedding)) in passages.iter().enumerate() {
                statement.execute(params![
                    recording_id,
                    ord as i64,
                    passage.start_ms,
                    passage.end_ms,
                    passage.text,
                    model,
                    to_vector(embedding),
                ])?;
            }
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn unindexed(&self, model: &str) -> Result<Vec<String>, RecordingsError> {
        let mut statement = self.conn.prepare(
            "SELECT t.recording_id
             FROM transcripts t
             WHERE NOT EXISTS (
                 SELECT 1 FROM passages p
                 WHERE p.recording_id = t.recording_id AND p.model = ?1
             )
             ORDER BY (SELECT started_at FROM recordings WHERE id = t.recording_id) DESC",
        )?;

        let rows = statement.query_map([model], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn index_size(&self, model: &str) -> Result<(u32, u32), RecordingsError> {
        Ok(self.conn.query_row(
            "SELECT COUNT(DISTINCT recording_id), COUNT(*) FROM passages WHERE model = ?1",
            [model],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    }

    pub fn nearest_passages(
        &self,
        embedding: &[f32],
        model: &str,
        limit: usize,
    ) -> Result<Vec<PassageHit>, RecordingsError> {
        if embedding.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let mut statement = self.conn.prepare(
            "SELECT p.recording_id, r.title, r.started_at, p.start_ms, p.end_ms, p.text, p.embedding
             FROM passages p
             JOIN recordings r ON r.id = p.recording_id
             WHERE p.model = ?1",
        )?;

        let rows = statement.query_map([model], |row| {
            let stored: Vec<u8> = row.get(6)?;
            Ok(PassageHit {
                recording_id: row.get(0)?,
                title: row.get(1)?,
                started_at: row.get(2)?,
                start_ms: row.get(3)?,
                end_ms: row.get(4)?,
                text: row.get(5)?,
                score: zscribe_core::similarity(embedding, &from_vector(&stored)),
            })
        })?;

        let mut hits = rows.collect::<Result<Vec<_>, _>>()?;
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(limit);
        Ok(hits)
    }

    pub fn get(&self, recording_id: &str) -> Result<Option<RecordingDetail>, RecordingsError> {
        let recording = self
            .conn
            .query_row(
                "SELECT r.id, r.started_at, r.duration_ms, r.source, r.template_id, r.title,
                        r.audio_path,
                        t.recording_id IS NOT NULL,
                        s.recording_id IS NOT NULL
                 FROM recordings r
                 LEFT JOIN transcripts t ON t.recording_id = r.id
                 LEFT JOIN summaries   s ON s.recording_id = r.id
                 WHERE r.id = ?1",
                [recording_id],
                row_to_recording,
            )
            .optional()?;

        let Some(mut recording) = recording else {
            return Ok(None);
        };

        recording.tags = {
            let mut statement = self
                .conn
                .prepare("SELECT tag FROM tags WHERE recording_id = ?1 ORDER BY tag")?;
            let rows = statement.query_map([recording_id], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        Ok(Some(RecordingDetail {
            transcript: self.transcript(recording_id)?,
            summary: self.summary(recording_id)?,
            recording,
        }))
    }

    pub fn transcript(&self, recording_id: &str) -> Result<Option<Transcript>, RecordingsError> {
        let row: Option<(String, String, String)> = self
            .conn
            .query_row(
                "SELECT language, model, segments FROM transcripts WHERE recording_id = ?1",
                [recording_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let Some((language, model, segments)) = row else {
            return Ok(None);
        };
        let segments: Vec<Segment> = serde_json::from_str(&segments)?;

        Ok(Some(Transcript {
            language,
            model,
            segments,
        }))
    }

    pub fn summary(&self, recording_id: &str) -> Result<Option<Summary>, RecordingsError> {
        let row: Option<SummaryRow> = self
            .conn
            .query_row(
                "SELECT provider, model, template_id, body_md, action_items,
                        input_tokens, output_tokens, elapsed_ms, redacted
                 FROM summaries WHERE recording_id = ?1",
                [recording_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .optional()?;

        let Some((
            provider,
            model,
            template_id,
            body_md,
            items,
            input,
            output,
            elapsed_ms,
            redacted,
        )) = row
        else {
            return Ok(None);
        };
        let action_items: Vec<ActionItem> = serde_json::from_str(&items)?;

        Ok(Some(Summary {
            provider,
            model,
            template_id,
            body_md,
            action_items,
            usage: TokenUsage { input, output },
            elapsed_ms,
            redacted,
        }))
    }

    pub fn delete(&self, recording_id: &str) -> Result<Option<PathBuf>, RecordingsError> {
        let audio = self.audio_path(recording_id)?;
        self.conn
            .execute("DELETE FROM recordings WHERE id = ?1", [recording_id])?;
        Ok(audio)
    }

    pub fn delete_all(&self) -> Result<Vec<PathBuf>, RecordingsError> {
        let audio = self.all_audio_paths()?;
        self.conn.execute("DELETE FROM recordings", [])?;
        Ok(audio)
    }

    pub fn forget_audio(&self, recording_id: &str) -> Result<Option<PathBuf>, RecordingsError> {
        let audio = self.audio_path(recording_id)?;
        self.conn.execute(
            "UPDATE recordings SET audio_path = NULL WHERE id = ?1",
            [recording_id],
        )?;
        Ok(audio)
    }

    fn audio_path(&self, recording_id: &str) -> Result<Option<PathBuf>, RecordingsError> {
        let path: Option<Option<String>> = self
            .conn
            .query_row(
                "SELECT audio_path FROM recordings WHERE id = ?1",
                [recording_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(path.flatten().map(PathBuf::from))
    }

    pub fn note_path(&self, recording_id: &str) -> Result<Option<PathBuf>, RecordingsError> {
        let path: Option<Option<String>> = self
            .conn
            .query_row(
                "SELECT note_path FROM recordings WHERE id = ?1",
                [recording_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(path.flatten().map(PathBuf::from))
    }

    pub fn set_note_path(&self, recording_id: &str, path: &Path) -> Result<(), RecordingsError> {
        self.conn.execute(
            "UPDATE recordings SET note_path = ?2 WHERE id = ?1",
            params![recording_id, path.display().to_string()],
        )?;
        Ok(())
    }

    fn all_audio_paths(&self) -> Result<Vec<PathBuf>, RecordingsError> {
        let mut statement = self
            .conn
            .prepare("SELECT audio_path FROM recordings WHERE audio_path IS NOT NULL")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(PathBuf::from)
            .collect())
    }

    pub fn count(&self) -> Result<u32, RecordingsError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM recordings", [], |row| row.get(0))?)
    }
}

pub const MATCH_OPEN: &str = "\u{e000}";
pub const MATCH_CLOSE: &str = "\u{e001}";

fn fts_query(input: &str) -> Option<String> {
    let words: Vec<String> = input
        .split_whitespace()
        .map(|word| word.replace('"', " ").trim().to_owned())
        .filter(|word| !word.is_empty())
        .collect();

    if words.is_empty() {
        return None;
    }

    let last = words.len() - 1;
    let terms: Vec<String> = words
        .iter()
        .enumerate()
        .map(|(index, word)| {
            if index == last {
                format!("\"{word}\"*")
            } else {
                format!("\"{word}\"")
            }
        })
        .collect();

    Some(terms.join(" AND "))
}

fn to_vector(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn from_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn row_to_recording(row: &rusqlite::Row<'_>) -> rusqlite::Result<Recording> {
    Ok(Recording {
        id: row.get(0)?,
        started_at: row.get(1)?,
        duration_ms: row.get(2)?,
        source: row.get(3)?,
        template_id: row.get(4)?,
        title: row.get(5)?,
        audio_path: row.get(6)?,
        has_transcript: row.get(7)?,
        has_summary: row.get(8)?,
        tags: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_recording(id: &str, started_at: i64) -> NewRecording {
        NewRecording {
            id: id.to_owned(),
            started_at,
            duration_ms: 30_000,
            source: "microphone".to_owned(),
            template_id: "meeting".to_owned(),
            title: "Planning call".to_owned(),
            audio_path: Some(format!("/data/recordings/{id}.wav")),
        }
    }

    fn transcript() -> Transcript {
        Transcript {
            language: "en".to_owned(),
            model: "large-v3-turbo".to_owned(),
            segments: vec![
                Segment::new(0, 2000, "Hello."),
                Segment::new(2000, 5000, "Let us ship it."),
            ],
        }
    }

    fn summary() -> Summary {
        Summary {
            provider: "ollama".to_owned(),
            model: "qwen2.5:7b".to_owned(),
            template_id: "meeting".to_owned(),
            body_md: "## Decisions\n\n- Ship it".to_owned(),
            action_items: vec![ActionItem {
                task: "Send the contract".to_owned(),
                owner: Some("Anna".to_owned()),
                due: None,
            }],
            usage: TokenUsage {
                input: 900,
                output: 120,
            },
            elapsed_ms: 4200,
            redacted: 0,
        }
    }

    fn store() -> Recordings {
        Recordings::in_memory().expect("open")
    }

    #[test]
    fn a_fresh_database_is_empty_and_at_the_current_schema() {
        let store = store();
        assert_eq!(store.count().expect("count"), 0);
        assert!(store.list(50).expect("list").is_empty());
    }

    #[test]
    fn opening_an_existing_database_twice_does_not_re_run_migrations() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("recordings.db");

        let first = Recordings::open(&path).expect("open");
        first.insert(&new_recording("a", 100)).expect("insert");
        drop(first);

        let second = Recordings::open(&path).expect("reopen");
        assert_eq!(second.count().expect("count"), 1);
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("recordings.db");

        let conn = Connection::open(&path).expect("open");
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .expect("bump");
        drop(conn);

        assert!(matches!(
            Recordings::open(&path),
            Err(RecordingsError::FromTheFuture { .. })
        ));
    }

    #[test]
    fn a_recording_round_trips_with_its_transcript_and_summary() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");
        store
            .set_transcript("a", &transcript())
            .expect("transcript");
        store.set_summary("a", &summary()).expect("summary");

        let detail = store.get("a").expect("get").expect("present");
        assert_eq!(detail.recording.title, "Planning call");
        assert_eq!(
            detail
                .transcript
                .as_ref()
                .expect("transcript")
                .segments
                .len(),
            2
        );
        assert_eq!(
            detail.summary.as_ref().expect("summary").action_items[0]
                .owner
                .as_deref(),
            Some("Anna")
        );
        assert_eq!(detail.summary.expect("summary").usage.total(), 1020);
    }

    #[test]
    fn an_unknown_id_reads_as_none_rather_than_erroring() {
        assert!(store().get("never-existed").expect("get").is_none());
    }

    #[test]
    fn the_list_is_newest_first() {
        let store = store();
        store.insert(&new_recording("old", 100)).expect("insert");
        store.insert(&new_recording("new", 900)).expect("insert");

        let ids: Vec<String> = store
            .list(50)
            .expect("list")
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, vec!["new", "old"]);
    }

    #[test]
    fn the_list_reports_what_each_recording_already_has() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");
        store.insert(&new_recording("b", 200)).expect("insert");
        store
            .set_transcript("b", &transcript())
            .expect("transcript");

        let rows = store.list(50).expect("list");
        let b = rows.iter().find(|r| r.id == "b").expect("b");
        let a = rows.iter().find(|r| r.id == "a").expect("a");

        assert!(b.has_transcript && !b.has_summary);
        assert!(!a.has_transcript && !a.has_summary);
    }

    #[test]
    fn re_transcribing_replaces_rather_than_duplicating() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");
        store.set_transcript("a", &transcript()).expect("first");

        let better = Transcript {
            model: "large-v3".to_owned(),
            segments: vec![Segment::new(0, 5000, "Hello, let us ship it.")],
            ..transcript()
        };
        store.set_transcript("a", &better).expect("second");

        let stored = store.transcript("a").expect("get").expect("present");
        assert_eq!(stored.model, "large-v3");
        assert_eq!(stored.segments.len(), 1);
    }

    #[test]
    fn the_full_text_column_is_derived_from_the_segments() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");
        store
            .set_transcript("a", &transcript())
            .expect("transcript");

        let text: String = store
            .conn
            .query_row(
                "SELECT full_text FROM transcripts WHERE recording_id = 'a'",
                [],
                |r| r.get(0),
            )
            .expect("query");
        assert_eq!(text, transcript().text());
    }

    #[test]
    fn correcting_a_line_rewrites_the_text_search_will_read() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");
        store
            .set_transcript("a", &transcript())
            .expect("transcript");

        let mut corrected = store.transcript("a").expect("read").expect("some");
        corrected.segments[0].text = "Marc speaking, not Max.".to_owned();
        store.set_transcript("a", &corrected).expect("save");

        let stored = store.transcript("a").expect("read").expect("some");
        assert_eq!(stored.segments[0].text, "Marc speaking, not Max.");

        assert_eq!(
            stored.segments[0].start_ms,
            transcript().segments[0].start_ms
        );
        assert_eq!(stored.segments.len(), transcript().segments.len());

        let text: String = store
            .conn
            .query_row(
                "SELECT full_text FROM transcripts WHERE recording_id = 'a'",
                [],
                |r| r.get(0),
            )
            .expect("query");
        assert!(text.contains("Marc speaking"), "got: {text}");
        assert!(
            !text.contains(&transcript().segments[0].text),
            "got: {text}"
        );
    }

    fn spoken(store: &Recordings, id: &str, started_at: i64, title: &str, said: &str) {
        let mut recording = new_recording(id, started_at);
        recording.title = title.to_owned();
        store.insert(&recording).expect("insert");

        store
            .set_transcript(
                id,
                &Transcript {
                    language: "en".to_owned(),
                    model: "large-v3-turbo".to_owned(),
                    segments: vec![Segment {
                        start_ms: 0,
                        end_ms: 4_000,
                        text: said.to_owned(),
                        speaker: None,
                    }],
                },
            )
            .expect("transcript");
    }

    #[test]
    fn search_finds_a_word_that_was_only_ever_spoken() {
        let store = store();
        spoken(
            &store,
            "a",
            100,
            "Tuesday sync",
            "We agreed the budget lands in March.",
        );
        spoken(
            &store,
            "b",
            200,
            "Design review",
            "The spacing is too tight.",
        );

        let hits = store.search("budget", 20).expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].recording.id, "a");
        assert!(
            hits[0].snippet.contains(MATCH_OPEN),
            "{:?}",
            hits[0].snippet
        );
        assert!(
            hits[0].snippet.contains("budget"),
            "the snippet quotes the passage: {:?}",
            hits[0].snippet
        );
    }

    #[test]
    fn search_reaches_titles_and_summaries_as_well_as_speech() {
        let store = store();
        spoken(&store, "a", 100, "Kickoff", "Nothing relevant was said.");
        store.set_summary("a", &summary()).expect("summary");

        assert_eq!(store.search("Kickoff", 20).expect("search").len(), 1);

        let word = summary()
            .body_md
            .split_whitespace()
            .find(|word| word.len() > 4)
            .expect("a word in the fixture")
            .to_owned();
        assert_eq!(store.search(&word, 20).expect("search").len(), 1);
    }

    #[test]
    fn a_correction_is_searchable_and_the_mistake_is_not() {
        let store = store();
        spoken(&store, "a", 100, "Sync", "Then Max will send the notes.");
        assert_eq!(store.search("Max", 20).expect("search").len(), 1);

        let mut fixed = store.transcript("a").expect("read").expect("some");
        fixed.segments[0].text = "Then Marc will send the notes.".to_owned();
        store.set_transcript("a", &fixed).expect("save");

        assert!(store.search("Max", 20).expect("search").is_empty());
        assert_eq!(store.search("Marc", 20).expect("search").len(), 1);
    }

    #[test]
    fn renaming_and_deleting_keep_the_index_in_step() {
        let store = store();
        spoken(&store, "a", 100, "Old name", "Something was said here.");

        store.set_title("a", "New name").expect("rename");
        assert!(store.search("Old", 20).expect("search").is_empty());
        assert_eq!(store.search("New", 20).expect("search").len(), 1);

        assert_eq!(store.search("Something", 20).expect("search").len(), 1);

        store.delete("a").expect("delete");
        assert!(store.search("Something", 20).expect("search").is_empty());
    }

    #[test]
    fn searching_is_forgiving_about_what_is_typed_into_it() {
        let store = store();
        spoken(
            &store,
            "a",
            100,
            "Sync",
            "The Q3 budget (revised) is agreed.",
        );

        for query in [
            "budget",
            "BUDGET",
            "budg",
            "\"budget\"",
            "budget AND",
            "(revised)",
            "q3 budget",
            "-budget",
            "budget*",
            "NEAR(",
            ":",
        ] {
            store
                .search(query, 20)
                .unwrap_or_else(|err| panic!("{query:?} should not error: {err}"));
        }

        assert!(store
            .search("budget spacing", 20)
            .expect("search")
            .is_empty());
        assert_eq!(store.search("budget agreed", 20).expect("search").len(), 1);
    }

    #[test]
    fn an_empty_search_answers_with_nothing_rather_than_everything() {
        let store = store();
        spoken(&store, "a", 100, "Sync", "Something was said.");

        assert!(store.search("", 20).expect("search").is_empty());
        assert!(store.search("   ", 20).expect("search").is_empty());
        assert!(store.search("\"\"", 20).expect("search").is_empty());
    }

    #[test]
    fn a_name_is_found_however_it_is_accented() {
        let store = store();
        spoken(&store, "a", 100, "Sync", "Müller said the café was closed.");

        assert_eq!(store.search("muller", 20).expect("search").len(), 1);
        assert_eq!(store.search("cafe", 20).expect("search").len(), 1);
    }

    #[test]
    fn a_database_from_before_search_existed_is_indexed_on_upgrade() {
        let store = store();
        spoken(&store, "a", 100, "Sync", "The budget lands in March.");

        store
            .conn
            .execute_batch(
                "ALTER TABLE summaries  DROP COLUMN redacted;
                 ALTER TABLE recordings DROP COLUMN note_path;",
            )
            .expect("drop the columns");

        store
            .conn
            .execute_batch("DROP TABLE search; DROP TABLE passages; DROP TABLE tags;")
            .expect("drop");

        for trigger in [
            "search_after_recording_insert",
            "search_after_recording_rename",
            "search_after_recording_delete",
            "search_after_transcript_insert",
            "search_after_transcript_update",
            "search_after_summary_insert",
            "search_after_summary_update",
        ] {
            store
                .conn
                .execute_batch(&format!("DROP TRIGGER {trigger}"))
                .expect("drop trigger");
        }

        store
            .conn
            .pragma_update(None, "user_version", 1)
            .expect("rewind");

        store.migrate().expect("migrate");

        assert_eq!(store.search("budget", 20).expect("search").len(), 1);

        assert_eq!(store.index_size("any-model").expect("size"), (0, 0));
    }

    fn passage(text: &str, start_ms: u32) -> zscribe_core::Passage {
        zscribe_core::Passage {
            start_ms,
            end_ms: start_ms + 60_000,
            text: text.to_owned(),
        }
    }

    #[test]
    fn a_question_finds_the_passage_closest_to_it_in_meaning() {
        let store = store();
        spoken(&store, "a", 100, "Budget call", "We agreed the money.");
        spoken(&store, "b", 200, "Design review", "The spacing is tight.");

        store
            .set_passages(
                "a",
                "test-model",
                &[(passage("We agreed the money.", 0), vec![1.0, 0.0])],
            )
            .expect("index a");
        store
            .set_passages(
                "b",
                "test-model",
                &[(passage("The spacing is tight.", 0), vec![0.0, 1.0])],
            )
            .expect("index b");

        let hits = store
            .nearest_passages(&[0.9, 0.1], "test-model", 5)
            .expect("search");

        assert_eq!(hits.len(), 2, "both are scored, best first");
        assert_eq!(hits[0].recording_id, "a");
        assert_eq!(hits[0].title, "Budget call");
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn an_index_from_another_model_is_never_mixed_in() {
        let store = store();
        spoken(&store, "a", 100, "Budget call", "We agreed the money.");
        store
            .set_passages(
                "a",
                "old-model",
                &[(passage("We agreed the money.", 0), vec![1.0, 0.0])],
            )
            .expect("index");

        assert!(store
            .nearest_passages(&[1.0, 0.0], "new-model", 5)
            .expect("search")
            .is_empty());
        assert_eq!(store.index_size("new-model").expect("size"), (0, 0));
        assert_eq!(store.index_size("old-model").expect("size"), (1, 1));
        assert_eq!(store.unindexed("new-model").expect("unindexed"), vec!["a"]);
        assert!(store.unindexed("old-model").expect("unindexed").is_empty());
    }

    #[test]
    fn re_indexing_replaces_rather_than_accumulating() {
        let store = store();
        spoken(&store, "a", 100, "Sync", "Something was said.");

        for _ in 0..3 {
            store
                .set_passages(
                    "a",
                    "m",
                    &[(passage("Something was said.", 0), vec![1.0, 0.0])],
                )
                .expect("index");
        }

        assert_eq!(store.index_size("m").expect("size"), (1, 1));
    }

    #[test]
    fn correcting_a_transcript_throws_away_the_vectors_it_was_embedded_from() {
        let store = store();
        spoken(&store, "a", 100, "Sync", "Then Max will send the notes.");
        store
            .set_passages(
                "a",
                "m",
                &[(passage("Then Max will send the notes.", 0), vec![1.0, 0.0])],
            )
            .expect("index");
        assert_eq!(store.index_size("m").expect("size"), (1, 1));

        let mut fixed = store.transcript("a").expect("read").expect("some");
        fixed.segments[0].text = "Then Marc will send the notes.".to_owned();
        store.set_transcript("a", &fixed).expect("save");

        assert_eq!(store.index_size("m").expect("size"), (0, 0));
        assert_eq!(store.unindexed("m").expect("unindexed"), vec!["a"]);
    }

    #[test]
    fn deleting_a_recording_takes_its_passages_with_it() {
        let store = store();
        spoken(&store, "a", 100, "Sync", "Something was said.");
        store
            .set_passages(
                "a",
                "m",
                &[(passage("Something was said.", 0), vec![1.0, 0.0])],
            )
            .expect("index");

        store.delete("a").expect("delete");
        assert_eq!(store.index_size("m").expect("size"), (0, 0));
    }

    #[test]
    fn an_embedding_survives_the_round_trip_through_the_database() {
        let original = vec![0.5f32, -0.25, 1.0, 0.0, -1.5];
        assert_eq!(from_vector(&to_vector(&original)), original);

        assert_eq!(from_vector(&[0, 0, 0]), Vec::<f32>::new());
    }

    #[test]
    fn tags_are_filed_replaced_and_counted() {
        let store = store();
        spoken(&store, "a", 100, "Client call", "We agreed the money.");
        spoken(&store, "b", 200, "Design review", "The spacing is tight.");

        store
            .set_tags("a", &["client".to_owned(), "billable".to_owned()])
            .expect("tag");
        store.set_tags("b", &["client".to_owned()]).expect("tag");

        assert_eq!(
            store.tags().expect("tags"),
            vec![("client".to_owned(), 2), ("billable".to_owned(), 1)]
        );

        store.set_tags("a", &["archive".to_owned()]).expect("retag");
        let listed = store.list(10).expect("list");
        let a = listed.iter().find(|r| r.id == "a").expect("a");
        assert_eq!(a.tags, vec!["archive".to_owned()]);

        let detail = store.get("a").expect("get").expect("some");
        assert_eq!(detail.recording.tags, vec!["archive".to_owned()]);
    }

    #[test]
    fn a_tag_is_tidied_rather_than_taken_literally() {
        let store = store();
        spoken(&store, "a", 100, "Sync", "Something was said.");

        store
            .set_tags(
                "a",
                &[
                    "  client  ".to_owned(),
                    String::new(),
                    "   ".to_owned(),
                    "CLIENT".to_owned(),
                    "billable".to_owned(),
                ],
            )
            .expect("tag");

        assert_eq!(
            store.get("a").expect("get").expect("some").recording.tags,
            vec!["billable".to_owned(), "client".to_owned()]
        );
    }

    #[test]
    fn a_tag_can_be_searched_for_like_any_other_word() {
        let store = store();
        spoken(
            &store,
            "a",
            100,
            "Tuesday call",
            "Nothing about money at all.",
        );
        store.set_tags("a", &["invoices".to_owned()]).expect("tag");

        let hits = store.search("invoices", 20).expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].recording.id, "a");

        store.set_tags("a", &[]).expect("untag");
        assert!(store.search("invoices", 20).expect("search").is_empty());
    }

    #[test]
    fn deleting_a_recording_takes_its_tags_with_it() {
        let store = store();
        spoken(&store, "a", 100, "Sync", "Something was said.");
        store.set_tags("a", &["client".to_owned()]).expect("tag");

        store.delete("a").expect("delete");
        assert!(store.tags().expect("tags").is_empty());
    }

    #[test]
    fn deleting_a_recording_hands_back_its_audio_to_unlink() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");

        let audio = store.delete("a").expect("delete").expect("audio path");
        assert!(audio.ends_with("a.wav"));
        assert_eq!(store.count().expect("count"), 0);
    }

    #[test]
    fn deleting_a_recording_takes_its_transcript_and_summary_with_it() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");
        store
            .set_transcript("a", &transcript())
            .expect("transcript");
        store.set_summary("a", &summary()).expect("summary");

        store.delete("a").expect("delete");

        assert!(store.transcript("a").expect("transcript").is_none());
        assert!(store.summary("a").expect("summary").is_none());
    }

    #[test]
    fn delete_all_returns_every_audio_file() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");
        store.insert(&new_recording("b", 200)).expect("insert");

        let audio = store.delete_all().expect("delete all");
        assert_eq!(audio.len(), 2);
        assert_eq!(store.count().expect("count"), 0);
    }

    #[test]
    fn delete_all_does_not_report_audio_that_was_already_removed() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");
        store
            .insert(&NewRecording {
                audio_path: None,
                ..new_recording("b", 200)
            })
            .expect("insert");

        assert_eq!(store.delete_all().expect("delete all").len(), 1);
    }

    #[test]
    fn forgetting_the_audio_keeps_the_text() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");
        store
            .set_transcript("a", &transcript())
            .expect("transcript");

        let audio = store.forget_audio("a").expect("forget").expect("path");
        assert!(audio.ends_with("a.wav"));

        let detail = store.get("a").expect("get").expect("present");
        assert_eq!(detail.recording.audio_path, None);
        assert!(detail.transcript.is_some());
    }

    #[test]
    fn a_recording_can_be_retitled() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");
        store.set_title("a", "Q3 planning").expect("retitle");

        assert_eq!(
            store
                .get("a")
                .expect("get")
                .expect("present")
                .recording
                .title,
            "Q3 planning"
        );
    }

    #[test]
    fn the_duration_can_be_corrected_after_the_recording_is_finalised() {
        let store = store();
        store.insert(&new_recording("a", 100)).expect("insert");
        store.set_duration("a", 125_000).expect("set duration");

        assert_eq!(
            store
                .get("a")
                .expect("get")
                .expect("present")
                .recording
                .duration_ms,
            125_000
        );
    }

    #[test]
    fn the_list_limit_is_honoured() {
        let store = store();
        for i in 0..10 {
            store
                .insert(&new_recording(&format!("r{i}"), i))
                .expect("insert");
        }
        assert_eq!(store.list(3).expect("list").len(), 3);
    }
}
