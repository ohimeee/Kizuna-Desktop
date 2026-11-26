//! Local library database (SQLite). Novels, chapters, categories, reading history and settings.
//!
//! Chapter order: `source_order` is the chapter's index in the plugin's natural reading order
//! (0 = chapter 1). Android stores the reverse because Mihon's sort machinery expects
//! manga-extension newest-first order (see novel-reader-ui-notes.md, "Chapter switching");
//! there's no such machinery here, so the natural order is stored directly.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::plugins::{ChapterItem, NovelDetails};

const SCHEMA_VERSION: i64 = 3;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS novels (
    id            INTEGER PRIMARY KEY,
    source_id     TEXT    NOT NULL,
    url           TEXT    NOT NULL,
    title         TEXT    NOT NULL,
    cover         TEXT,
    author        TEXT,
    description   TEXT,
    genres        TEXT    NOT NULL DEFAULT '[]',
    status        TEXT,
    favorite      INTEGER NOT NULL DEFAULT 0,
    initialized   INTEGER NOT NULL DEFAULT 0,
    date_added    INTEGER NOT NULL DEFAULT 0,
    last_update   INTEGER NOT NULL DEFAULT 0,
    UNIQUE (source_id, url)
);
CREATE INDEX IF NOT EXISTS novels_favorite ON novels(favorite);

CREATE TABLE IF NOT EXISTS chapters (
    id             INTEGER PRIMARY KEY,
    novel_id       INTEGER NOT NULL REFERENCES novels(id) ON DELETE CASCADE,
    url            TEXT    NOT NULL,
    name           TEXT    NOT NULL,
    chapter_number REAL    NOT NULL DEFAULT -1,
    date_upload    INTEGER NOT NULL DEFAULT 0,
    source_order   INTEGER NOT NULL,
    read           INTEGER NOT NULL DEFAULT 0,
    bookmark       INTEGER NOT NULL DEFAULT 0,
    locked         INTEGER NOT NULL DEFAULT 0,
    last_paragraph INTEGER NOT NULL DEFAULT 0,
    progress       INTEGER NOT NULL DEFAULT 0,
    date_fetch     INTEGER NOT NULL DEFAULT 0,
    UNIQUE (novel_id, url)
);
CREATE INDEX IF NOT EXISTS chapters_novel ON chapters(novel_id, source_order);
CREATE INDEX IF NOT EXISTS chapters_fetch ON chapters(date_fetch);

CREATE TABLE IF NOT EXISTS categories (
    id    INTEGER PRIMARY KEY,
    name  TEXT    NOT NULL,
    sort  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS novel_categories (
    novel_id    INTEGER NOT NULL REFERENCES novels(id) ON DELETE CASCADE,
    category_id INTEGER NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    PRIMARY KEY (novel_id, category_id)
);

CREATE TABLE IF NOT EXISTS history (
    chapter_id INTEGER PRIMARY KEY REFERENCES chapters(id) ON DELETE CASCADE,
    last_read  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS history_last_read ON history(last_read);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub struct Db {
    conn: Mutex<Connection>,
}

// ---- row types -------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Novel {
    pub id: i64,
    pub source_id: String,
    pub url: String,
    pub title: String,
    pub cover: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub status: Option<String>,
    pub favorite: bool,
    pub initialized: bool,
    pub date_added: i64,
    pub last_update: i64,
    /// "novel" or "manga".
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub id: i64,
    pub novel_id: i64,
    pub url: String,
    pub name: String,
    pub chapter_number: f64,
    pub date_upload: i64,
    pub source_order: i64,
    pub read: bool,
    pub bookmark: bool,
    pub locked: bool,
    pub last_paragraph: i64,
    pub progress: i64,
    pub date_fetch: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    #[serde(flatten)]
    pub novel: Novel,
    pub unread: i64,
    pub total: i64,
    pub last_read: i64,
    pub category_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub sort: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub chapter: Chapter,
    pub novel_title: String,
    pub novel_cover: Option<String>,
    pub last_read: i64,
    pub kind: String,
    pub source_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateItem {
    pub chapter: Chapter,
    pub novel_title: String,
    pub novel_cover: Option<String>,
    pub kind: String,
    pub source_id: String,
}

const NOVEL_COLS: &str = "n.id, n.source_id, n.url, n.title, n.cover, n.author, n.description, n.genres, \
     n.status, n.favorite, n.initialized, n.date_added, n.last_update, n.kind";

const CHAPTER_COLS: &str = "c.id, c.novel_id, c.url, c.name, c.chapter_number, c.date_upload, c.source_order, \
     c.read, c.bookmark, c.locked, c.last_paragraph, c.progress, c.date_fetch";

fn novel_from(row: &rusqlite::Row, o: usize) -> rusqlite::Result<Novel> {
    let genres: String = row.get(o + 7)?;
    Ok(Novel {
        id: row.get(o)?,
        source_id: row.get(o + 1)?,
        url: row.get(o + 2)?,
        title: row.get(o + 3)?,
        cover: row.get(o + 4)?,
        author: row.get(o + 5)?,
        description: row.get(o + 6)?,
        genres: serde_json::from_str(&genres).unwrap_or_default(),
        status: row.get(o + 8)?,
        favorite: row.get(o + 9)?,
        initialized: row.get(o + 10)?,
        date_added: row.get(o + 11)?,
        last_update: row.get(o + 12)?,
        kind: row.get(o + 13)?,
    })
}

fn chapter_from(row: &rusqlite::Row, o: usize) -> rusqlite::Result<Chapter> {
    Ok(Chapter {
        id: row.get(o)?,
        novel_id: row.get(o + 1)?,
        url: row.get(o + 2)?,
        name: row.get(o + 3)?,
        chapter_number: row.get(o + 4)?,
        date_upload: row.get(o + 5)?,
        source_order: row.get(o + 6)?,
        read: row.get(o + 7)?,
        bookmark: row.get(o + 8)?,
        locked: row.get(o + 9)?,
        last_paragraph: row.get(o + 10)?,
        progress: row.get(o + 11)?,
        date_fetch: row.get(o + 12)?,
    })
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;
        // v2: manga live in the same table as novels.
        let has_kind: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('novels') WHERE name = 'kind'")?
            .exists([])?;
        if !has_kind {
            conn.execute_batch("ALTER TABLE novels ADD COLUMN kind TEXT NOT NULL DEFAULT 'novel';")?;
        }
        // v3: manga extensions' private `memo` (extension-lib 1.6), handed back on later calls.
        for table in ["novels", "chapters"] {
            let has_memo: bool = conn
                .prepare(&format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name = 'memo'"))?
                .exists([])?;
            if !has_memo {
                conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN memo TEXT NOT NULL DEFAULT '{{}}';"))?;
            }
        }
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("db mutex poisoned")
    }

    // ---- novels ------------------------------------------------------------------------

    /// Finds or creates the local row for a novel seen in a source listing; refreshes its
    /// title/cover while it isn't in the library (library entries keep their stored data).
    pub fn upsert_listed_novel(&self, source_id: &str, url: &str, title: &str, cover: Option<&str>) -> Result<i64> {
        self.upsert_listed(source_id, url, title, cover, "novel")
    }

    pub fn upsert_listed(&self, source_id: &str, url: &str, title: &str, cover: Option<&str>, kind: &str) -> Result<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO novels (source_id, url, title, cover, kind) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(source_id, url) DO UPDATE SET
               title = CASE WHEN favorite = 0 THEN excluded.title ELSE title END,
               cover = CASE WHEN favorite = 0 AND excluded.cover IS NOT NULL THEN excluded.cover ELSE cover END",
            params![source_id, url, title, cover, kind],
        )?;
        Ok(conn.query_row(
            "SELECT id FROM novels WHERE source_id = ?1 AND url = ?2",
            params![source_id, url],
            |r| r.get(0),
        )?)
    }

    pub fn novel(&self, id: i64) -> Result<Novel> {
        let conn = self.conn();
        conn.query_row(&format!("SELECT {NOVEL_COLS} FROM novels n WHERE n.id = ?1"), [id], |r| novel_from(r, 0))
            .with_context(|| format!("novel {id} not found"))
    }

    /// Favorite flags for a page of source results, keyed by url.
    pub fn favorites_among(&self, source_id: &str, urls: &[String]) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT 1 FROM novels WHERE source_id = ?1 AND url = ?2 AND favorite = 1")?;
        let mut out = Vec::new();
        for u in urls {
            if stmt.exists(params![source_id, u])? {
                out.push(u.clone());
            }
        }
        Ok(out)
    }

    pub fn apply_details(&self, id: i64, d: &NovelDetails) -> Result<()> {
        let genres = d.genres.as_ref().map(|g| serde_json::to_string(g).unwrap_or_else(|_| "[]".into()));
        self.conn().execute(
            "UPDATE novels SET
               title = COALESCE(NULLIF(?2, ''), title),
               cover = COALESCE(NULLIF(?3, ''), cover),
               author = COALESCE(?4, author),
               description = COALESCE(?5, description),
               genres = COALESCE(?6, genres),
               status = COALESCE(?7, status),
               initialized = 1,
               last_update = ?8
             WHERE id = ?1",
            params![id, d.title, d.cover, d.author, d.description, genres, d.status, now_ms()],
        )?;
        Ok(())
    }

    /// A manga's extension memo (JSON object text, `{}` when none).
    pub fn novel_memo(&self, id: i64) -> Result<String> {
        Ok(self.conn().query_row("SELECT memo FROM novels WHERE id = ?1", [id], |r| r.get(0))?)
    }

    pub fn set_novel_memo(&self, id: i64, memo: &str) -> Result<()> {
        self.conn().execute("UPDATE novels SET memo = ?2 WHERE id = ?1", params![id, memo])?;
        Ok(())
    }

    pub fn chapter_memo(&self, id: i64) -> Result<String> {
        Ok(self.conn().query_row("SELECT memo FROM chapters WHERE id = ?1", [id], |r| r.get(0))?)
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) -> Result<()> {
        self.conn().execute(
            "UPDATE novels SET favorite = ?2,
               date_added = CASE WHEN ?2 = 1 AND favorite = 0 THEN ?3 ELSE date_added END
             WHERE id = ?1",
            params![id, favorite, now_ms()],
        )?;
        Ok(())
    }

    // ---- chapters ----------------------------------------------------------------------

    pub fn chapters(&self, novel_id: i64) -> Result<Vec<Chapter>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {CHAPTER_COLS} FROM chapters c WHERE c.novel_id = ?1 ORDER BY c.source_order"
        ))?;
        let rows = stmt.query_map([novel_id], |r| chapter_from(r, 0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn chapter(&self, id: i64) -> Result<Chapter> {
        let conn = self.conn();
        conn.query_row(&format!("SELECT {CHAPTER_COLS} FROM chapters c WHERE c.id = ?1"), [id], |r| chapter_from(r, 0))
            .with_context(|| format!("chapter {id} not found"))
    }

    /// Syncs a freshly fetched chapter list (natural order). Keeps read/bookmark/progress of
    /// chapters that still exist, removes chapters the source dropped, and stamps new ones
    /// with `date_fetch` (what the Updates screen lists). Returns how many were new.
    pub fn sync_chapters(&self, novel_id: i64, fetched: &[ChapterItem]) -> Result<usize> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let had_any: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM chapters WHERE novel_id = ?1)", [novel_id], |r| r.get(0))?;
        let now = now_ms();
        let mut new_count = 0;
        {
            let mut exists = tx.prepare_cached("SELECT 1 FROM chapters WHERE novel_id = ?1 AND url = ?2")?;
            let mut upsert = tx.prepare_cached(
                "INSERT INTO chapters (novel_id, url, name, chapter_number, date_upload, source_order, locked, date_fetch, memo)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, COALESCE(?9, '{}'))
                 ON CONFLICT(novel_id, url) DO UPDATE SET
                   name = excluded.name, chapter_number = excluded.chapter_number,
                   date_upload = excluded.date_upload, source_order = excluded.source_order,
                   locked = excluded.locked, memo = COALESCE(?9, memo)",
            )?;
            for (i, c) in fetched.iter().enumerate() {
                let is_new = !exists.exists(params![novel_id, c.url])?;
                // On the very first fetch nothing is "new" to the user — don't flood Updates.
                let fetch_time = if is_new && had_any { now } else { 0 };
                if is_new && had_any {
                    new_count += 1;
                }
                upsert.execute(params![
                    novel_id, c.url, c.name, c.chapter_number, c.date_upload, i as i64, c.locked, fetch_time, c.memo
                ])?;
            }
        }
        // Drop chapters no longer listed by the source.
        let urls: Vec<&str> = fetched.iter().map(|c| c.url.as_str()).collect();
        let json = serde_json::to_string(&urls)?;
        tx.execute(
            "DELETE FROM chapters WHERE novel_id = ?1 AND url NOT IN (SELECT value FROM json_each(?2))",
            params![novel_id, json],
        )?;
        tx.commit()?;
        Ok(new_count)
    }

    pub fn set_read(&self, ids: &[i64], read: bool) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE chapters SET read = ?2, last_paragraph = CASE WHEN ?2 = 0 THEN 0 ELSE last_paragraph END,
                   progress = CASE WHEN ?2 = 1 THEN 100 ELSE 0 END WHERE id = ?1",
            )?;
            for id in ids {
                stmt.execute(params![id, read])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_bookmark(&self, ids: &[i64], bookmark: bool) -> Result<()> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("UPDATE chapters SET bookmark = ?2 WHERE id = ?1")?;
        for id in ids {
            stmt.execute(params![id, bookmark])?;
        }
        Ok(())
    }

    /// Saves the reader position. `mark_read` only ever sets read (never clears it), matching
    /// Android: reaching the end marks read; scrolling back doesn't unmark.
    pub fn save_progress(&self, chapter_id: i64, paragraph: i64, percent: i64, mark_read: bool) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE chapters SET last_paragraph = ?2, progress = ?3,
               read = CASE WHEN ?4 = 1 THEN 1 ELSE read END WHERE id = ?1",
            params![chapter_id, paragraph, percent.clamp(0, 100), mark_read],
        )?;
        conn.execute(
            "INSERT INTO history (chapter_id, last_read) VALUES (?1, ?2)
             ON CONFLICT(chapter_id) DO UPDATE SET last_read = excluded.last_read",
            params![chapter_id, now_ms()],
        )?;
        Ok(())
    }

    // ---- library -----------------------------------------------------------------------

    pub fn library(&self) -> Result<Vec<LibraryItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {NOVEL_COLS},
               (SELECT COUNT(*) FROM chapters c WHERE c.novel_id = n.id AND c.read = 0),
               (SELECT COUNT(*) FROM chapters c WHERE c.novel_id = n.id),
               COALESCE((SELECT MAX(h.last_read) FROM history h JOIN chapters c ON c.id = h.chapter_id
                         WHERE c.novel_id = n.id), 0),
               COALESCE((SELECT json_group_array(category_id) FROM novel_categories nc WHERE nc.novel_id = n.id), '[]')
             FROM novels n WHERE n.favorite = 1"
        ))?;
        let rows = stmt.query_map([], |r| {
            let cats: String = r.get(17)?;
            Ok(LibraryItem {
                novel: novel_from(r, 0)?,
                unread: r.get(14)?,
                total: r.get(15)?,
                last_read: r.get(16)?,
                category_ids: serde_json::from_str(&cats).unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn categories(&self) -> Result<Vec<Category>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT id, name, sort FROM categories ORDER BY sort, id")?;
        let rows = stmt.query_map([], |r| Ok(Category { id: r.get(0)?, name: r.get(1)?, sort: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn create_category(&self, name: &str) -> Result<Category> {
        let conn = self.conn();
        let sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort), -1) + 1 FROM categories", [], |r| r.get(0))?;
        conn.execute("INSERT INTO categories (name, sort) VALUES (?1, ?2)", params![name, sort])?;
        Ok(Category { id: conn.last_insert_rowid(), name: name.to_string(), sort })
    }

    pub fn rename_category(&self, id: i64, name: &str) -> Result<()> {
        self.conn().execute("UPDATE categories SET name = ?2 WHERE id = ?1", params![id, name])?;
        Ok(())
    }

    pub fn delete_category(&self, id: i64) -> Result<()> {
        self.conn().execute("DELETE FROM categories WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn reorder_categories(&self, ids: &[i64]) -> Result<()> {
        let conn = self.conn();
        for (i, id) in ids.iter().enumerate() {
            conn.execute("UPDATE categories SET sort = ?2 WHERE id = ?1", params![id, i as i64])?;
        }
        Ok(())
    }

    pub fn set_novel_categories(&self, novel_id: i64, category_ids: &[i64]) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM novel_categories WHERE novel_id = ?1", [novel_id])?;
        for c in category_ids {
            tx.execute("INSERT OR IGNORE INTO novel_categories (novel_id, category_id) VALUES (?1, ?2)", params![novel_id, c])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn novel_categories(&self, novel_id: i64) -> Result<Vec<i64>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT category_id FROM novel_categories WHERE novel_id = ?1")?;
        let rows = stmt.query_map([novel_id], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---- history / updates -------------------------------------------------------------

    pub fn history(&self, limit: i64) -> Result<Vec<HistoryItem>> {
        let conn = self.conn();
        // One row per novel: its most recently read chapter (like Mihon's History).
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {CHAPTER_COLS}, n.title, n.cover, h.last_read, n.kind, n.source_id
             FROM history h
             JOIN chapters c ON c.id = h.chapter_id
             JOIN novels n ON n.id = c.novel_id
             WHERE h.last_read = (SELECT MAX(h2.last_read) FROM history h2 JOIN chapters c2 ON c2.id = h2.chapter_id
                                  WHERE c2.novel_id = c.novel_id)
             ORDER BY h.last_read DESC LIMIT ?1"
        ))?;
        let rows = stmt.query_map([limit], |r| {
            Ok(HistoryItem {
                chapter: chapter_from(r, 0)?,
                novel_title: r.get(13)?,
                novel_cover: r.get(14)?,
                last_read: r.get(15)?,
                kind: r.get(16)?,
                source_id: r.get(17)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn remove_history(&self, novel_id: i64) -> Result<()> {
        self.conn().execute(
            "DELETE FROM history WHERE chapter_id IN (SELECT id FROM chapters WHERE novel_id = ?1)",
            [novel_id],
        )?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<()> {
        self.conn().execute("DELETE FROM history", [])?;
        Ok(())
    }

    pub fn updates(&self, limit: i64) -> Result<Vec<UpdateItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {CHAPTER_COLS}, n.title, n.cover, n.kind, n.source_id FROM chapters c JOIN novels n ON n.id = c.novel_id
             WHERE n.favorite = 1 AND c.date_fetch > 0
             ORDER BY c.date_fetch DESC, c.source_order DESC LIMIT ?1"
        ))?;
        let rows = stmt.query_map([limit], |r| {
            Ok(UpdateItem { chapter: chapter_from(r, 0)?, novel_title: r.get(13)?, novel_cover: r.get(14)?, kind: r.get(15)?, source_id: r.get(16)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---- settings ----------------------------------------------------------------------

    pub fn settings(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut map = serde_json::Map::new();
        for row in rows {
            let (k, v) = row?;
            map.insert(k, serde_json::from_str(&v).unwrap_or(serde_json::Value::Null));
        }
        Ok(map)
    }

    pub fn set_setting(&self, key: &str, value: &serde_json::Value) -> Result<()> {
        self.conn().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value.to_string()],
        )?;
        Ok(())
    }

    pub fn novel_by_source_url(&self, source_id: &str, url: &str) -> Result<Option<i64>> {
        Ok(self
            .conn()
            .query_row("SELECT id FROM novels WHERE source_id = ?1 AND url = ?2", params![source_id, url], |r| r.get(0))
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(url: &str) -> ChapterItem {
        ChapterItem { name: url.into(), url: url.into(), chapter_number: -1.0, date_upload: 0, locked: false, memo: None }
    }

    #[test]
    fn memos_are_stored_and_kept() {
        let dir = std::env::temp_dir().join(format!("kizuna-test-memo-{}.db", now_ms()));
        let db = Db::open(&dir).unwrap();
        let id = db.upsert_listed("9", "/m", "M", None, "manga").unwrap();
        assert_eq!(db.novel_memo(id).unwrap(), "{}");
        db.set_novel_memo(id, r#"{"k":1}"#).unwrap();
        assert_eq!(db.novel_memo(id).unwrap(), r#"{"k":1}"#);

        let mut a = ch("a");
        a.memo = Some(r#"{"p":"x"}"#.into());
        db.sync_chapters(id, &[a, ch("b")]).unwrap();
        let chapters = db.chapters(id).unwrap();
        assert_eq!(db.chapter_memo(chapters[0].id).unwrap(), r#"{"p":"x"}"#);
        assert_eq!(db.chapter_memo(chapters[1].id).unwrap(), "{}");
        db.sync_chapters(id, &[ch("a"), ch("b")]).unwrap();
        assert_eq!(db.chapter_memo(chapters[0].id).unwrap(), r#"{"p":"x"}"#, "None keeps the stored memo");
        drop(db);
        let _ = std::fs::remove_file(dir);
    }

    #[test]
    fn sync_keeps_state_and_flags_new() {
        let dir = std::env::temp_dir().join(format!("kizuna-test-{}.db", now_ms()));
        let db = Db::open(&dir).unwrap();
        let id = db.upsert_listed_novel("1", "/n", "N", None).unwrap();

        assert_eq!(db.sync_chapters(id, &[ch("a"), ch("b")]).unwrap(), 0, "first fetch isn't 'new'");
        let a = db.chapters(id).unwrap()[0].id;
        db.save_progress(a, 5, 40, true).unwrap();

        assert_eq!(db.sync_chapters(id, &[ch("a"), ch("b"), ch("c")]).unwrap(), 1);
        let chapters = db.chapters(id).unwrap();
        assert_eq!(chapters.len(), 3);
        assert!(chapters[0].read && chapters[0].last_paragraph == 5);
        assert!(chapters[2].date_fetch > 0 && chapters[0].date_fetch == 0);

        db.sync_chapters(id, &[ch("b"), ch("c")]).unwrap();
        assert_eq!(db.chapters(id).unwrap().len(), 2, "dropped chapter removed");
        assert_eq!(db.history(10).unwrap().len(), 0, "history cascades with the chapter");
        let _ = std::fs::remove_file(dir);
    }
}
