//! Offline chapter downloads: a single background worker fetching chapters one at a time
//! (politely, with a short pause between chapters), saved under readable names:
//!
//!   <app data>/downloads/<title>/<chapter name>.txt       novels
//!   <app data>/downloads/<title>/<chapter name>/000.webp   manga (one image per page)
//!
//! Each chapter's path, relative to the downloads folder, is stored in the database
//! (`chapters.download_path`), so a title the source renames later, or two novels with the same
//! title, never lose track of their files. Files are written under a `.part` name and renamed
//! when complete. The readers use a saved copy first.
//!
//! Progress is pushed to the UI as `download-progress` events.

use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::AppState;
use crate::db::{Chapter, Db, Novel};
use crate::plugins;

const PAUSE_BETWEEN_CHAPTERS: Duration = Duration::from_millis(400);
const PAUSE_BETWEEN_PAGES: Duration = Duration::from_millis(150);
/// Folder and file name length limit, so paths stay well under Windows' 260 characters.
const MAX_NAME: usize = 80;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub chapter_id: i64,
    pub novel_id: i64,
    pub chapter_name: String,
    pub novel_title: String,
}

#[derive(Debug, Clone, Serialize)]
// rename_all only renames the variants; rename_all_fields makes `chapter_id` -> `chapterId`,
// which is what the UI reads.
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "state")]
pub enum Progress {
    Queued { chapter_id: i64 },
    Downloading { chapter_id: i64 },
    /// Pages saved so far (manga chapters), for the progress ring.
    Progress { chapter_id: i64, done: usize, total: usize },
    Done { chapter_id: i64, novel_id: i64 },
    Failed { chapter_id: i64, error: String },
    Removed { chapter_id: i64 },
}

#[derive(Default)]
struct Inner {
    queue: VecDeque<QueueItem>,
    active: Option<QueueItem>,
    paused: bool,
}

pub struct Downloads {
    /// The downloads folder; the user can move it (More -> Library -> Download location).
    dir: Mutex<PathBuf>,
    inner: Mutex<Inner>,
    wake: Condvar,
}

// ---- names ----------------------------------------------------------------------------------

/// A title or chapter name as a safe Windows file name: no `<>:"/\|?*` or control characters,
/// no trailing dots or spaces, not a reserved device name, at most MAX_NAME characters.
pub fn safe_name(raw: &str, fallback: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut name: String = collapsed.chars().take(MAX_NAME).collect();
    while name.ends_with(['.', ' ']) {
        name.pop();
    }
    let name = name.trim_start().to_string();
    if name.is_empty() {
        return fallback.to_string();
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit());
    if reserved { format!("{name}_") } else { name }
}

/// A stored relative path is only ever plain names joined by `/` (never `..`, a drive or a root).
fn is_plain_relative(rel: &str) -> bool {
    !rel.is_empty() && Path::new(rel).components().all(|c| matches!(c, Component::Normal(_)))
}

impl Downloads {
    pub fn new(dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir: Mutex::new(dir), inner: Mutex::new(Inner::default()), wake: Condvar::new() })
    }

    pub fn dir(&self) -> PathBuf {
        self.dir.lock().unwrap().clone()
    }

    /// Moves the downloads folder (and everything in it) to `new`. Refused while anything is
    /// downloading. Stored paths are relative, so nothing in the database changes.
    pub fn move_to(&self, new: &Path) -> Result<()> {
        {
            let inner = self.inner.lock().unwrap();
            anyhow::ensure!(inner.active.is_none() && inner.queue.is_empty(), "Wait for the download queue to finish first");
        }
        let old = self.dir();
        std::fs::create_dir_all(new).with_context(|| format!("couldn't create {}", new.display()))?;
        let (old_c, new_c) = (old.canonicalize()?, new.canonicalize()?);
        if old_c == new_c {
            return Ok(());
        }
        anyhow::ensure!(!new_c.starts_with(&old_c), "Pick a folder outside the current downloads folder");
        for entry in std::fs::read_dir(&old)?.flatten() {
            let dest = new.join(entry.file_name());
            anyhow::ensure!(!dest.exists(), "{} already has a \"{}\"", new.display(), entry.file_name().to_string_lossy());
            if std::fs::rename(entry.path(), &dest).is_err() {
                // Another drive: copy, then remove the original.
                copy_all(&entry.path(), &dest)?;
                remove_any(&entry.path())?;
            }
        }
        *self.dir.lock().unwrap() = new.to_path_buf();
        Ok(())
    }

    fn abs(&self, rel: &str) -> Option<PathBuf> {
        is_plain_relative(rel).then(|| self.dir().join(rel))
    }

    /// Where a chapter's download lives, if it has one on disk.
    fn saved(&self, db: &Db, chapter_id: i64) -> Option<PathBuf> {
        let rel = db.download_path(chapter_id).ok()??;
        self.abs(&rel).filter(|p| p.exists())
    }

    pub fn is_downloaded(&self, db: &Db, chapter_id: i64) -> bool {
        self.saved(db, chapter_id).is_some()
    }

    pub fn read(&self, db: &Db, chapter_id: i64) -> Option<String> {
        let p = self.saved(db, chapter_id)?;
        if p.is_file() { std::fs::read_to_string(p).ok() } else { None }
    }

    /// A downloaded manga chapter's page files, in reading order.
    pub fn pages(&self, db: &Db, chapter_id: i64) -> Option<Vec<String>> {
        let dir = self.saved(db, chapter_id).filter(|p| p.is_dir())?;
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        (!names.is_empty()).then_some(names)
    }

    /// A page file from a `<chapter id>/<file>` reference (the kimg route's `l=`), or None if
    /// it isn't a page of a downloaded chapter.
    pub fn page_file(&self, db: &Db, reference: &str) -> Option<PathBuf> {
        let (chapter, file) = reference.split_once('/')?;
        let chapter: i64 = chapter.parse().ok()?;
        let safe_file = !file.is_empty() && !file.starts_with('.') && file.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.');
        if !safe_file {
            return None;
        }
        let dir = self.saved(db, chapter).filter(|p| p.is_dir())?;
        Some(dir.join(file))
    }

    /// Chapter ids of a novel that have a saved copy.
    pub fn downloaded(&self, db: &Db, novel_id: i64) -> Vec<i64> {
        db.download_paths(novel_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, rel)| self.abs(rel).is_some_and(|p| p.exists()))
            .map(|(id, _)| id)
            .collect()
    }

    pub fn delete(&self, db: &Db, chapter_ids: &[i64]) -> Result<()> {
        for &id in chapter_ids {
            if let Some(p) = db.download_path(id)?.and_then(|rel| self.abs(&rel)) {
                remove_any(&p)?;
                if let Some(parent) = p.parent().filter(|d| *d != self.dir()) {
                    let _ = std::fs::remove_dir(parent); // only if now empty
                }
            }
            db.set_download_path(id, None)?;
        }
        Ok(())
    }

    pub fn delete_novel(&self, db: &Db, novel_id: i64) -> Result<()> {
        let ids: Vec<i64> = db.download_paths(novel_id)?.into_iter().map(|(id, _)| id).collect();
        self.delete(db, &ids)
    }

    /// Picks the paths for a new download: the novel's folder (reused if it already has
    /// downloads, so a renamed title keeps one folder) and a chapter name not already taken.
    fn target(&self, db: &Db, novel: &Novel, chapter: &Chapter, ext: Option<&str>) -> Result<String> {
        let folder = match db.novel_download_folder(novel.id)? {
            Some(f) if is_plain_relative(&f) => f,
            _ => {
                let base = safe_name(&novel.title, &format!("Novel {}", novel.id));
                // Another novel (same title, other source) already has this folder.
                let taken = db.download_folder_owner(&base)?.is_some_and(|owner| owner != novel.id);
                if taken { format!("{base} ({})", novel.id) } else { base }
            }
        };
        let base = safe_name(&chapter.name, &format!("Chapter {}", chapter.id));
        let with_ext = |n: &str| match ext {
            Some(e) => format!("{folder}/{n}.{e}"),
            None => format!("{folder}/{n}"),
        };
        let first = with_ext(&base);
        let taken = |rel: &str| -> Result<bool> {
            Ok(db.download_path_owner(rel)?.is_some_and(|owner| owner != chapter.id)
                || (self.dir().join(rel).exists() && db.download_path(chapter.id)?.as_deref() != Some(rel)))
        };
        Ok(if taken(&first)? { with_ext(&format!("{base} ({})", chapter.id)) } else { first })
    }

    /// Moves downloads saved in the old `<novel id>/<chapter id>` layout to readable names.
    /// Runs at startup; anything it can't place is left where it is.
    pub fn migrate_numeric(&self, db: &Db) {
        let Ok(novels) = std::fs::read_dir(self.dir()) else { return };
        for novel_dir in novels.flatten() {
            let name = novel_dir.file_name().to_string_lossy().into_owned();
            if !novel_dir.path().is_dir() || name.parse::<i64>().is_err() {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(novel_dir.path()) else { continue };
            for e in entries.flatten() {
                let file = e.file_name().to_string_lossy().into_owned();
                if file.ends_with(".part") {
                    let _ = remove_any(&e.path());
                    continue;
                }
                let is_text = file.ends_with(".txt");
                let Ok(chapter_id) = file.trim_end_matches(".txt").parse::<i64>() else { continue };
                let (Ok(chapter), true) = (db.chapter(chapter_id), is_text || e.path().is_dir()) else { continue };
                let Ok(novel) = db.novel(chapter.novel_id) else { continue };
                let Ok(rel) = self.target(db, &novel, &chapter, is_text.then_some("txt")) else { continue };
                let dest = self.dir().join(&rel);
                let moved = dest.parent().map(std::fs::create_dir_all).transpose().is_ok() && std::fs::rename(e.path(), &dest).is_ok();
                if moved {
                    let _ = db.set_download_path(chapter_id, Some(&rel));
                }
            }
            let _ = std::fs::remove_dir(novel_dir.path()); // only if now empty
        }
    }

    /// Total bytes used by downloads.
    pub fn size(&self) -> u64 {
        fn walk(p: &Path) -> u64 {
            std::fs::read_dir(p)
                .map(|it| {
                    it.flatten()
                        .map(|e| match e.metadata() {
                            Ok(m) if m.is_dir() => walk(&e.path()),
                            Ok(m) => m.len(),
                            Err(_) => 0,
                        })
                        .sum()
                })
                .unwrap_or(0)
        }
        walk(&self.dir())
    }

    pub fn enqueue(&self, db: &Db, items: Vec<QueueItem>, app: &AppHandle) {
        let mut inner = self.inner.lock().unwrap();
        for it in items {
            let dup = inner.queue.iter().any(|q| q.chapter_id == it.chapter_id)
                || inner.active.as_ref().is_some_and(|a| a.chapter_id == it.chapter_id)
                || self.is_downloaded(db, it.chapter_id);
            if !dup {
                let _ = app.emit("download-progress", Progress::Queued { chapter_id: it.chapter_id });
                inner.queue.push_back(it);
            }
        }
        self.wake.notify_all();
    }

    pub fn cancel(&self, chapter_ids: &[i64], app: &AppHandle) {
        let mut inner = self.inner.lock().unwrap();
        inner.queue.retain(|q| {
            let keep = !chapter_ids.contains(&q.chapter_id);
            if !keep {
                let _ = app.emit("download-progress", Progress::Removed { chapter_id: q.chapter_id });
            }
            keep
        });
    }

    pub fn clear(&self, app: &AppHandle) {
        let ids: Vec<i64> = self.inner.lock().unwrap().queue.iter().map(|q| q.chapter_id).collect();
        self.cancel(&ids, app);
    }

    pub fn set_paused(&self, paused: bool) {
        self.inner.lock().unwrap().paused = paused;
        self.wake.notify_all();
    }

    pub fn snapshot(&self) -> (Option<QueueItem>, Vec<QueueItem>, bool) {
        let inner = self.inner.lock().unwrap();
        (inner.active.clone(), inner.queue.iter().cloned().collect(), inner.paused)
    }

    /// Starts the worker thread. Call once, after the state is managed by Tauri.
    pub fn start(app: AppHandle) {
        std::thread::Builder::new()
            .name("downloads".into())
            .spawn(move || {
                let state = app.state::<Arc<AppState>>().inner().clone();
                let dl = &state.downloads;
                loop {
                    let item = {
                        let mut inner = dl.inner.lock().unwrap();
                        while inner.paused || inner.queue.is_empty() {
                            inner = dl.wake.wait(inner).unwrap();
                        }
                        let item = inner.queue.pop_front().unwrap();
                        inner.active = Some(item.clone());
                        item
                    };
                    let _ = app.emit("download-progress", Progress::Downloading { chapter_id: item.chapter_id });
                    let chapter_id = item.chapter_id;
                    let progress = |done: usize, total: usize| {
                        let _ = app.emit("download-progress", Progress::Progress { chapter_id, done, total });
                    };
                    let result = fetch_and_save(&state, &item, &progress);
                    dl.inner.lock().unwrap().active = None;
                    let event = match result {
                        Ok(()) => Progress::Done { chapter_id: item.chapter_id, novel_id: item.novel_id },
                        Err(e) => Progress::Failed { chapter_id: item.chapter_id, error: format!("{e:#}") },
                    };
                    let _ = app.emit("download-progress", event);
                    std::thread::sleep(PAUSE_BETWEEN_CHAPTERS);
                }
            })
            .expect("spawning download worker");
    }
}

fn copy_all(from: &Path, to: &Path) -> Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)?.flatten() {
            copy_all(&e.path(), &to.join(e.file_name()))?;
        }
    } else {
        std::fs::copy(from, to).with_context(|| format!("copying {}", from.display()))?;
    }
    Ok(())
}

fn remove_any(p: &Path) -> Result<()> {
    if p.is_dir() {
        std::fs::remove_dir_all(p).with_context(|| format!("deleting {}", p.display()))?;
    } else if p.exists() {
        std::fs::remove_file(p).with_context(|| format!("deleting {}", p.display()))?;
    }
    Ok(())
}

fn part_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push(".part");
    PathBuf::from(s)
}

fn fetch_and_save(state: &AppState, item: &QueueItem, progress: &dyn Fn(usize, usize)) -> Result<()> {
    let chapter = state.db.chapter(item.chapter_id)?;
    let novel = state.db.novel(chapter.novel_id)?;
    if novel.kind == "manga" {
        return save_manga_chapter(state, &novel, &chapter, progress);
    }
    let path = state.plugins.path_for(&novel.source_id)?;
    let text = plugins::content(&path, &chapter.url)?;
    anyhow::ensure!(!text.trim().is_empty(), "the source returned an empty chapter");
    let rel = state.downloads.target(&state.db, &novel, &chapter, Some("txt"))?;
    let out = state.downloads.dir().join(&rel);
    std::fs::create_dir_all(out.parent().unwrap())?;
    // Write then rename, so a half-written file never counts as downloaded.
    let tmp = part_path(&out);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &out)?;
    state.db.set_download_path(chapter.id, Some(&rel))?;
    Ok(())
}

/// Saves every page of a manga chapter through the extension host (each source's own headers).
fn save_manga_chapter(state: &AppState, novel: &Novel, chapter: &Chapter, progress: &dyn Fn(usize, usize)) -> Result<()> {
    let memo = state.db.chapter_memo(chapter.id)?;
    let pages = crate::manga::pages(&state.manga, &novel.source_id, &chapter.url, &memo)?;
    anyhow::ensure!(!pages.is_empty(), "the source returned no pages");
    let rel = state.downloads.target(&state.db, novel, chapter, None)?;
    let out = state.downloads.dir().join(&rel);
    let tmp = part_path(&out);
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    progress(0, pages.len());
    for (i, p) in pages.iter().enumerate() {
        let url = match p.image_url.as_deref().filter(|u| !u.is_empty()) {
            Some(u) => u.to_string(),
            None => crate::manga::image_url(&state.manga, &novel.source_id, &p.url)?,
        };
        let raw = tmp.join(format!("{i:03}.img"));
        crate::manga::image(&state.manga, &novel.source_id, &url, &raw)
            .with_context(|| format!("page {} of {}", i + 1, pages.len()))?;
        let ext = crate::manga::images::extension(&std::fs::read(&raw)?);
        std::fs::rename(&raw, tmp.join(format!("{i:03}.{ext}")))?;
        progress(i + 1, pages.len());
        std::thread::sleep(PAUSE_BETWEEN_PAGES);
    }
    let _ = std::fs::remove_dir_all(&out);
    std::fs::rename(&tmp, &out)?;
    state.db.set_download_path(chapter.id, Some(&rel))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_events_use_camel_case_fields() {
        let json = serde_json::to_value(Progress::Progress { chapter_id: 7, done: 2, total: 9 }).unwrap();
        assert_eq!(json, serde_json::json!({ "state": "progress", "chapterId": 7, "done": 2, "total": 9 }));
        let json = serde_json::to_value(Progress::Done { chapter_id: 7, novel_id: 3 }).unwrap();
        assert_eq!(json, serde_json::json!({ "state": "done", "chapterId": 7, "novelId": 3 }));
    }

    #[test]
    fn names_are_safe_on_windows() {
        assert_eq!(safe_name("Ice Lord", "x"), "Ice Lord");
        assert_eq!(safe_name("Re:Zero? <Part 2> \"A|B\"*", "x"), "Re_Zero_ _Part 2_ _A_B__");
        assert_eq!(safe_name("  Chapter 12...  ", "x"), "Chapter 12");
        assert_eq!(safe_name("a/b\\c", "x"), "a_b_c");
        assert_eq!(safe_name("CON", "x"), "CON_");
        assert_eq!(safe_name("com1.txt", "x"), "com1.txt_");
        assert_eq!(safe_name("Comic", "x"), "Comic");
        assert_eq!(safe_name("...", "Fallback"), "Fallback");
        assert_eq!(safe_name(&"長".repeat(200), "x").chars().count(), MAX_NAME);
    }

    #[test]
    fn stored_paths_must_be_plain() {
        assert!(is_plain_relative("Ice Lord/Chapter 12"));
        for bad in ["", "../x", "Ice Lord/../../etc", "C:/Windows", "/abs", "\\\\server\\share"] {
            assert!(!is_plain_relative(bad), "{bad}");
        }
    }

    #[test]
    fn readable_targets_with_collisions_and_page_refs() {
        let root = std::env::temp_dir().join(format!("kizuna-dl-test-{}-{}", std::process::id(), crate::db::now_ms()));
        let d = Downloads::new(root.join("downloads")).unwrap();
        let db = Db::open(&root.join("t.db")).unwrap();
        let a = db.upsert_listed("1", "/a", "Ice: Lord", None, "manga").unwrap();
        let b = db.upsert_listed("2", "/b", "Ice: Lord", None, "manga").unwrap();
        let ch = |url: &str| crate::plugins::ChapterItem { name: "Chapter 12".into(), url: url.into(), chapter_number: 12.0, date_upload: 0, locked: false, memo: None };
        db.sync_chapters(a, &[ch("x"), ch("y")]).unwrap();
        db.sync_chapters(b, &[ch("z")]).unwrap();
        let (na, nb) = (db.novel(a).unwrap(), db.novel(b).unwrap());
        let ca = db.chapters(a).unwrap();
        let cb = db.chapters(b).unwrap();

        let first = d.target(&db, &na, &ca[0], None).unwrap();
        assert_eq!(first, "Ice_ Lord/Chapter 12");
        std::fs::create_dir_all(d.dir().join(&first)).unwrap();
        std::fs::write(d.dir().join(&first).join("000.webp"), b"x").unwrap();
        db.set_download_path(ca[0].id, Some(&first)).unwrap();

        // Same chapter name in the same manga, then the same title from another source.
        assert_eq!(d.target(&db, &na, &ca[1], None).unwrap(), format!("Ice_ Lord/Chapter 12 ({})", ca[1].id));
        assert_eq!(d.target(&db, &nb, &cb[0], None).unwrap(), format!("Ice_ Lord ({b})/Chapter 12"));

        // The kimg route only serves files of downloaded chapters.
        assert_eq!(d.pages(&db, ca[0].id).unwrap(), ["000.webp"]);
        assert!(d.page_file(&db, &format!("{}/000.webp", ca[0].id)).is_some());
        for bad in [format!("{}/../x", ca[0].id), format!("{}/.hidden", ca[0].id), format!("{}/000.webp", ca[1].id), "x/000.webp".into()] {
            assert!(d.page_file(&db, &bad).is_none(), "{bad}");
        }
        assert_eq!(d.downloaded(&db, a), [ca[0].id]);

        d.delete(&db, &[ca[0].id]).unwrap();
        assert!(!d.dir().join("Ice_ Lord").exists(), "empty title folder removed");
        assert!(d.downloaded(&db, a).is_empty());

        // Moving the folder keeps downloads working (paths are relative).
        db.set_download_path(ca[1].id, Some("Ice_ Lord/Chapter 12 b")).unwrap();
        std::fs::create_dir_all(d.dir().join("Ice_ Lord/Chapter 12 b")).unwrap();
        std::fs::write(d.dir().join("Ice_ Lord/Chapter 12 b/000.png"), b"x").unwrap();
        let moved = root.join("elsewhere");
        d.move_to(&moved).unwrap();
        assert_eq!(d.dir(), moved);
        assert_eq!(d.pages(&db, ca[1].id).unwrap(), ["000.png"]);
        assert!(d.move_to(&moved.join("inside")).is_err(), "can't move into itself");
        drop(db);
        let _ = std::fs::remove_dir_all(root);
    }
}
