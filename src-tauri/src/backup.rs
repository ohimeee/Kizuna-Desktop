//! Importing Kizuna Android backups (Mihon `.tachibk`: gzip + protobuf). Field numbers come from
//! the Android app's `data/backup/models/`; see `docs/phone-backup-and-sync-plan.md`.
//!
//! Merging never loses progress, so importing the same backup twice changes nothing: read and
//! bookmark stay set once set, the further reading position wins, history keeps the newer time,
//! favorites and categories are only ever added.

use std::collections::{HashMap, HashSet};
use std::io::Read;

use anyhow::{Context, Result, bail};
use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::db::{Db, now_ms};
use crate::manga;

// ---- protobuf wire reader -------------------------------------------------------------------

enum Wire<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32(u32),
}

struct Fields<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Fields<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn varint(&mut self) -> Result<u64> {
        let mut out = 0u64;
        for shift in (0..64).step_by(7) {
            let Some(&b) = self.buf.get(self.pos) else { bail!("truncated backup") };
            self.pos += 1;
            out |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(out);
            }
        }
        bail!("malformed backup (bad varint)")
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.buf.len()).context("truncated backup")?;
        let out = &self.buf[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    /// The next (field number, value), or None at the end.
    fn next(&mut self) -> Result<Option<(u64, Wire<'a>)>> {
        if self.pos >= self.buf.len() {
            return Ok(None);
        }
        let tag = self.varint()?;
        let value = match tag & 7 {
            0 => Wire::Varint(self.varint()?),
            1 => Wire::Fixed64(u64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            2 => {
                let n = self.varint()? as usize;
                Wire::Bytes(self.take(n)?)
            }
            5 => Wire::Fixed32(u32::from_le_bytes(self.take(4)?.try_into().unwrap())),
            t => bail!("malformed backup (wire type {t})"),
        };
        Ok(Some((tag >> 3, value)))
    }
}

fn int(w: &Wire) -> i64 {
    match w {
        Wire::Varint(v) | Wire::Fixed64(v) => *v as i64,
        Wire::Fixed32(v) => i64::from(*v),
        Wire::Bytes(_) => 0,
    }
}

fn text(w: &Wire) -> String {
    match w {
        Wire::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        _ => String::new(),
    }
}

/// Repeated integers, packed (one length-delimited run) or not (one field each).
fn ints(w: &Wire, out: &mut Vec<i64>) -> Result<()> {
    match w {
        Wire::Bytes(b) => {
            let mut f = Fields::new(b);
            while f.pos < b.len() {
                out.push(f.varint()? as i64);
            }
        }
        other => out.push(int(other)),
    }
    Ok(())
}

// ---- backup model (only what desktop uses) ------------------------------------------------

#[derive(Debug, Default)]
pub struct Backup {
    pub manga: Vec<BManga>,
    pub categories: Vec<BCategory>,
    /// source id -> name
    pub sources: HashMap<i64, String>,
    pub repos: Vec<String>,
}

#[derive(Debug, Default)]
pub struct BManga {
    pub source: i64,
    pub url: String,
    pub title: String,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub status: i32,
    pub thumbnail: Option<String>,
    pub date_added: i64,
    pub chapters: Vec<BChapter>,
    /// Category `order` values.
    pub categories: Vec<i64>,
    pub favorite: bool,
    pub history: Vec<(String, i64)>,
    pub initialized: bool,
    pub memo: Option<String>,
}

#[derive(Debug, Default)]
pub struct BChapter {
    pub url: String,
    pub name: String,
    pub read: bool,
    pub bookmark: bool,
    pub last_page_read: i64,
    pub date_upload: i64,
    pub chapter_number: f64,
    pub source_order: i64,
    pub memo: Option<String>,
}

#[derive(Debug, Default)]
pub struct BCategory {
    pub name: String,
    pub order: i64,
}

fn memo_of(w: &Wire) -> Option<String> {
    manga::memo_text(&manga::memo_value(&text(w)))
}

fn parse_manga(buf: &[u8]) -> Result<BManga> {
    let mut m = BManga { favorite: true, ..Default::default() };
    let mut f = Fields::new(buf);
    while let Some((n, w)) = f.next()? {
        match n {
            1 => m.source = int(&w),
            2 => m.url = text(&w),
            3 => m.title = text(&w),
            4 => m.artist = Some(text(&w)),
            5 => m.author = Some(text(&w)),
            6 => m.description = Some(text(&w)),
            7 => m.genres.push(text(&w)),
            8 => m.status = int(&w) as i32,
            9 => m.thumbnail = Some(text(&w)),
            13 => m.date_added = int(&w),
            16 => {
                if let Wire::Bytes(b) = w {
                    m.chapters.push(parse_chapter(b)?);
                }
            }
            17 => ints(&w, &mut m.categories)?,
            100 => m.favorite = int(&w) != 0,
            104 => {
                if let Wire::Bytes(b) = w {
                    let (mut url, mut at) = (String::new(), 0);
                    let mut h = Fields::new(b);
                    while let Some((n, w)) = h.next()? {
                        match n {
                            1 => url = text(&w),
                            2 => at = int(&w),
                            _ => {}
                        }
                    }
                    m.history.push((url, at));
                }
            }
            111 => m.initialized = int(&w) != 0,
            112 => m.memo = memo_of(&w),
            _ => {}
        }
    }
    Ok(m)
}

fn parse_chapter(buf: &[u8]) -> Result<BChapter> {
    let mut c = BChapter::default();
    let mut f = Fields::new(buf);
    while let Some((n, w)) = f.next()? {
        match n {
            1 => c.url = text(&w),
            2 => c.name = text(&w),
            4 => c.read = int(&w) != 0,
            5 => c.bookmark = int(&w) != 0,
            6 => c.last_page_read = int(&w),
            8 => c.date_upload = int(&w),
            9 => {
                if let Wire::Fixed32(bits) = w {
                    c.chapter_number = f64::from(f32::from_bits(bits));
                }
            }
            10 => c.source_order = int(&w),
            13 => c.memo = memo_of(&w),
            _ => {}
        }
    }
    Ok(c)
}

/// Parses a backup file's bytes (gzipped or not).
pub fn parse(bytes: &[u8]) -> Result<Backup> {
    let raw;
    let data: &[u8] = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes).read_to_end(&mut out).context("the backup file is damaged")?;
        raw = out;
        &raw
    } else if bytes.first() == Some(&b'{') {
        bail!("This is a JSON backup. Make a backup from Kizuna's Settings → Data and storage instead.");
    } else {
        bytes
    };
    let mut b = Backup::default();
    let mut f = Fields::new(data);
    let bad = || "this isn't a Kizuna or Mihon backup";
    while let Some((n, w)) = f.next().context(bad())? {
        let Wire::Bytes(body) = w else { continue };
        match n {
            1 => b.manga.push(parse_manga(body).context(bad())?),
            2 => {
                let mut c = BCategory::default();
                let mut g = Fields::new(body);
                while let Some((n, w)) = g.next()? {
                    match n {
                        1 => c.name = text(&w),
                        2 => c.order = int(&w),
                        _ => {}
                    }
                }
                b.categories.push(c);
            }
            101 => {
                let (mut name, mut id) = (String::new(), 0);
                let mut g = Fields::new(body);
                while let Some((n, w)) = g.next()? {
                    match n {
                        1 => name = text(&w),
                        2 => id = int(&w),
                        _ => {}
                    }
                }
                b.sources.insert(id, name);
            }
            106 => {
                let mut g = Fields::new(body);
                while let Some((n, w)) = g.next()? {
                    if n == 1 {
                        b.repos.push(text(&w));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(b)
}

// ---- import ---------------------------------------------------------------------------------

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub novels: usize,
    pub manga: usize,
    pub chapters: usize,
    pub history: usize,
    pub categories_created: usize,
    pub repos_added: usize,
    /// Sources the backup uses that aren't installed here.
    pub missing_sources: Vec<MissingSource>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingSource {
    pub name: String,
    /// "novel" or "manga".
    pub kind: String,
    /// Library entries using it.
    pub entries: usize,
}

/// What the importer knows about sources on this machine.
pub struct SourceKinds {
    /// Novel plugin source ids: installed plugins plus the whole plugin catalog.
    pub novel: HashSet<String>,
    pub installed_novel: HashSet<String>,
    pub installed_manga: HashSet<String>,
}

/// Merges a parsed backup into the library. Returns what was added.
pub fn import(db: &Db, backup: &Backup, kinds: &SourceKinds) -> Result<(ImportSummary, Vec<String>)> {
    let mut sum = ImportSummary::default();
    let mut missing: HashMap<(String, String), usize> = HashMap::new();
    let now = now_ms();

    db.transaction(|tx| {
        // Categories: by name, created when missing.
        let mut by_name: HashMap<String, i64> = HashMap::new();
        {
            let mut stmt = tx.prepare("SELECT id, name FROM categories")?;
            for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
                let (id, name) = row?;
                by_name.insert(name, id);
            }
        }
        let mut cat_by_order: HashMap<i64, i64> = HashMap::new();
        let mut cats: Vec<&BCategory> = backup.categories.iter().filter(|c| !c.name.is_empty()).collect();
        cats.sort_by_key(|c| c.order);
        for c in cats {
            let id = match by_name.get(&c.name) {
                Some(&id) => id,
                None => {
                    tx.execute(
                        "INSERT INTO categories (name, sort) VALUES (?1, (SELECT COALESCE(MAX(sort), -1) + 1 FROM categories))",
                        [&c.name],
                    )?;
                    sum.categories_created += 1;
                    let id = tx.last_insert_rowid();
                    by_name.insert(c.name.clone(), id);
                    id
                }
            };
            cat_by_order.insert(c.order, id);
        }

        for m in &backup.manga {
            if m.url.is_empty() {
                continue;
            }
            let source_id = m.source.to_string();
            let kind = if kinds.novel.contains(&source_id) { "novel" } else { "manga" };
            let installed = if kind == "novel" {
                kinds.installed_novel.contains(&source_id)
            } else {
                kinds.installed_manga.contains(&source_id)
            };
            if !installed {
                let name = backup.sources.get(&m.source).cloned().unwrap_or_else(|| format!("Source {source_id}"));
                *missing.entry((name, kind.to_string())).or_default() += 1;
            }

            let author = match (m.author.as_deref().filter(|a| !a.is_empty()), m.artist.as_deref().filter(|a| !a.is_empty())) {
                (Some(a), Some(b)) if a != b => Some(format!("{a}, {b}")),
                (a, b) => a.or(b).map(str::to_string),
            };
            let genres = serde_json::to_string(&m.genres)?;
            let date_added = if m.date_added > 0 { m.date_added } else { now };
            tx.execute(
                "INSERT INTO novels (source_id, url, title, cover, author, description, genres, status, favorite,
                                     initialized, date_added, kind, memo)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, CASE WHEN ?9 = 1 THEN ?11 ELSE 0 END, ?12, COALESCE(?13, '{}'))
                 ON CONFLICT(source_id, url) DO UPDATE SET
                   title = CASE WHEN initialized = 0 AND excluded.title <> '' THEN excluded.title ELSE title END,
                   cover = CASE WHEN initialized = 0 THEN COALESCE(excluded.cover, cover) ELSE cover END,
                   author = CASE WHEN initialized = 0 THEN COALESCE(excluded.author, author) ELSE author END,
                   description = CASE WHEN initialized = 0 THEN COALESCE(excluded.description, description) ELSE description END,
                   genres = CASE WHEN initialized = 0 AND excluded.genres <> '[]' THEN excluded.genres ELSE genres END,
                   status = CASE WHEN initialized = 0 THEN COALESCE(excluded.status, status) ELSE status END,
                   date_added = CASE WHEN favorite = 0 AND excluded.favorite = 1 THEN excluded.date_added ELSE date_added END,
                   favorite = MAX(favorite, excluded.favorite),
                   memo = CASE WHEN memo = '{}' THEN excluded.memo ELSE memo END",
                params![
                    source_id,
                    m.url,
                    if m.title.is_empty() { &m.url } else { &m.title },
                    m.thumbnail.as_deref().filter(|t| !t.is_empty()),
                    author,
                    m.description,
                    genres,
                    manga::status_text(m.status),
                    m.favorite,
                    m.initialized,
                    date_added,
                    kind,
                    m.memo,
                ],
            )?;
            let novel_id: i64 =
                tx.query_row("SELECT id FROM novels WHERE source_id = ?1 AND url = ?2", params![source_id, m.url], |r| r.get(0))?;
            if kind == "novel" {
                sum.novels += 1;
            } else {
                sum.manga += 1;
            }

            for order in &m.categories {
                if let Some(cat) = cat_by_order.get(order) {
                    tx.execute("INSERT OR IGNORE INTO novel_categories (novel_id, category_id) VALUES (?1, ?2)", params![novel_id, cat])?;
                }
            }

            // Android's sourceOrder is newest-first; desktop stores reading order.
            let mut chapters: Vec<&BChapter> = m.chapters.iter().filter(|c| !c.url.is_empty()).collect();
            chapters.sort_by_key(|c| std::cmp::Reverse(c.source_order));
            let mut ins = tx.prepare_cached(
                "INSERT INTO chapters (novel_id, url, name, chapter_number, date_upload, source_order, read, bookmark,
                                       last_paragraph, progress, date_fetch, memo)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, COALESCE(?11, '{}'))
                 ON CONFLICT(novel_id, url) DO UPDATE SET
                   read = MAX(read, excluded.read),
                   bookmark = MAX(bookmark, excluded.bookmark),
                   last_paragraph = MAX(last_paragraph, excluded.last_paragraph),
                   progress = MAX(progress, excluded.progress),
                   memo = CASE WHEN memo = '{}' THEN excluded.memo ELSE memo END",
            )?;
            for (i, c) in chapters.iter().enumerate() {
                ins.execute(params![
                    novel_id,
                    c.url,
                    if c.name.is_empty() { &c.url } else { &c.name },
                    c.chapter_number,
                    c.date_upload,
                    i as i64,
                    c.read,
                    c.bookmark,
                    c.last_page_read.max(0),
                    if c.read { 100 } else { 0 },
                    c.memo,
                ])?;
                sum.chapters += 1;
            }

            for (url, at) in &m.history {
                let chapter: Option<i64> = tx
                    .query_row("SELECT id FROM chapters WHERE novel_id = ?1 AND url = ?2", params![novel_id, url], |r| r.get(0))
                    .optional()?;
                if let (Some(chapter), true) = (chapter, *at > 0) {
                    tx.execute(
                        "INSERT INTO history (chapter_id, last_read) VALUES (?1, ?2)
                         ON CONFLICT(chapter_id) DO UPDATE SET last_read = MAX(last_read, excluded.last_read)",
                        params![chapter, at],
                    )?;
                    sum.history += 1;
                }
            }
        }
        Ok(())
    })?;

    let mut list: Vec<MissingSource> =
        missing.into_iter().map(|((name, kind), entries)| MissingSource { name, kind, entries }).collect();
    list.sort_by(|a, b| b.entries.cmp(&a.entries).then(a.name.cmp(&b.name)));
    sum.missing_sources = list;
    let repos = backup.repos.iter().filter(|r| !r.trim().is_empty()).cloned().collect();
    Ok((sum, repos))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal protobuf writer, to build backups the way Android's kotlinx ProtoBuf does.
    fn varint(out: &mut Vec<u8>, mut v: u64) {
        while v >= 0x80 {
            out.push((v as u8) | 0x80);
            v >>= 7;
        }
        out.push(v as u8);
    }
    fn vi(out: &mut Vec<u8>, field: u64, v: u64) {
        varint(out, field << 3);
        varint(out, v);
    }
    fn by(out: &mut Vec<u8>, field: u64, b: &[u8]) {
        varint(out, field << 3 | 2);
        varint(out, b.len() as u64);
        out.extend_from_slice(b);
    }
    fn f32f(out: &mut Vec<u8>, field: u64, v: f32) {
        varint(out, field << 3 | 5);
        out.extend_from_slice(&v.to_bits().to_le_bytes());
    }

    fn chapter(url: &str, order: u64, read: bool, page: u64) -> Vec<u8> {
        let mut c = Vec::new();
        by(&mut c, 1, url.as_bytes());
        by(&mut c, 2, format!("Ch {url}").as_bytes());
        vi(&mut c, 4, read as u64);
        vi(&mut c, 6, page);
        f32f(&mut c, 9, 1.5);
        vi(&mut c, 10, order);
        by(&mut c, 13, br#"{"p":1}"#);
        c
    }

    fn sample() -> Vec<u8> {
        let mut m = Vec::new();
        vi(&mut m, 1, 42);
        by(&mut m, 2, b"/novel/a");
        by(&mut m, 3, b"Novel A");
        by(&mut m, 5, b"Writer");
        by(&mut m, 7, b"Action");
        by(&mut m, 7, b"Drama");
        vi(&mut m, 8, 2);
        vi(&mut m, 13, 1_700_000_000_000);
        // Newest first, like Android: c3 (order 0), c2, c1.
        by(&mut m, 16, &chapter("c3", 0, false, 0));
        by(&mut m, 16, &chapter("c2", 1, false, 7));
        by(&mut m, 16, &chapter("c1", 2, true, 30));
        vi(&mut m, 17, 5); // category order 5, unpacked
        vi(&mut m, 100, 1);
        let mut h = Vec::new();
        by(&mut h, 1, b"c2");
        vi(&mut h, 2, 1_700_000_500_000);
        by(&mut m, 104, &h);
        vi(&mut m, 111, 1);
        by(&mut m, 112, br#"{"path":"/a"}"#);

        let mut b = Vec::new();
        by(&mut b, 1, &m);
        let mut cat = Vec::new();
        by(&mut cat, 1, b"Reading");
        vi(&mut cat, 2, 5);
        by(&mut b, 2, &cat);
        let mut src = Vec::new();
        by(&mut src, 1, b"Source A");
        vi(&mut src, 2, 42);
        by(&mut b, 101, &src);
        let mut store = Vec::new();
        by(&mut store, 1, b"https://repo.example/index.min.json");
        by(&mut store, 2, b"Example");
        by(&mut b, 106, &store);

        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut gz, &b).unwrap();
        gz.finish().unwrap()
    }

    fn kinds(novel: bool) -> SourceKinds {
        let ids: HashSet<String> = ["42".to_string()].into();
        SourceKinds {
            novel: if novel { ids.clone() } else { HashSet::new() },
            installed_novel: HashSet::new(),
            installed_manga: if novel { HashSet::new() } else { ids },
        }
    }

    #[test]
    fn parses_gzipped_backup() {
        let b = parse(&sample()).unwrap();
        assert_eq!(b.manga.len(), 1);
        let m = &b.manga[0];
        assert_eq!((m.source, m.url.as_str(), m.title.as_str(), m.status), (42, "/novel/a", "Novel A", 2));
        assert_eq!(m.genres, ["Action", "Drama"]);
        assert_eq!(m.chapters.len(), 3);
        assert_eq!(m.chapters[2].chapter_number, 1.5);
        assert_eq!(m.categories, [5]);
        assert_eq!(m.memo.as_deref(), Some(r#"{"path":"/a"}"#));
        assert_eq!(b.categories[0].name, "Reading");
        assert_eq!(b.sources[&42], "Source A");
        assert_eq!(b.repos, ["https://repo.example/index.min.json"]);
    }

    #[test]
    fn rejects_json_and_garbage() {
        assert!(parse(b"{\"backupManga\":[]}").unwrap_err().to_string().contains("JSON"));
        assert!(parse(&[0x0f, 0xff, 0xff]).is_err());
    }

    #[test]
    fn imports_and_merges_idempotently() {
        let path = std::env::temp_dir().join(format!("kizuna-backup-test-{}.db", now_ms()));
        let db = Db::open(&path).unwrap();
        let backup = parse(&sample()).unwrap();

        let (sum, repos) = import(&db, &backup, &kinds(true)).unwrap();
        assert_eq!((sum.novels, sum.manga, sum.chapters, sum.history, sum.categories_created), (1, 0, 3, 1, 1));
        assert_eq!(repos, ["https://repo.example/index.min.json"]);
        assert_eq!(sum.missing_sources[0].name, "Source A");

        let lib = db.library().unwrap();
        assert_eq!(lib.len(), 1);
        let n = &lib[0].novel;
        assert!(n.favorite && n.kind == "novel" && n.status.as_deref() == Some("Completed"));
        assert_eq!(n.date_added, 1_700_000_000_000);
        assert_eq!(lib[0].category_ids.len(), 1);
        assert_eq!(db.novel_memo(n.id).unwrap(), r#"{"path":"/a"}"#);

        // Reading order: c1, c2, c3.
        let ch = db.chapters(n.id).unwrap();
        assert_eq!(ch.iter().map(|c| c.url.as_str()).collect::<Vec<_>>(), ["c1", "c2", "c3"]);
        assert!(ch[0].read && ch[0].progress == 100 && ch[0].last_paragraph == 30);
        assert_eq!(ch[1].last_paragraph, 7);
        assert_eq!(db.chapter_memo(ch[0].id).unwrap(), r#"{"p":1}"#);
        assert_eq!(db.history(10).unwrap().len(), 1);

        // Local progress beyond the backup survives a second import, which adds nothing new.
        db.save_progress(ch[1].id, 50, 60, true).unwrap();
        let (again, _) = import(&db, &backup, &kinds(true)).unwrap();
        assert_eq!(again.categories_created, 0);
        let ch = db.chapters(n.id).unwrap();
        assert!(ch[1].read && ch[1].last_paragraph == 50);
        assert_eq!(db.library().unwrap().len(), 1);
        assert_eq!(db.categories().unwrap().len(), 1);
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn unknown_sources_import_as_manga() {
        let path = std::env::temp_dir().join(format!("kizuna-backup-test-m-{}.db", now_ms()));
        let db = Db::open(&path).unwrap();
        let (sum, _) = import(&db, &parse(&sample()).unwrap(), &kinds(false)).unwrap();
        assert_eq!((sum.novels, sum.manga), (0, 1));
        assert!(sum.missing_sources.is_empty(), "installed manga source isn't missing");
        assert_eq!(db.library().unwrap()[0].novel.kind, "manga");
        drop(db);
        let _ = std::fs::remove_file(path);
    }
}
