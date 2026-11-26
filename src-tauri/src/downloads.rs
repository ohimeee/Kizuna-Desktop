//! Offline chapter downloads: a single background worker fetching chapter text one at a time
//! (politely, with a short pause between chapters) and saving it under
//! `<app data>/downloads/<novel id>/<chapter id>.txt`. The reader reads a saved copy first.
//!
//! Progress is pushed to the UI as `download-progress` events.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::AppState;
use crate::plugins;

const PAUSE_BETWEEN_CHAPTERS: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub chapter_id: i64,
    pub novel_id: i64,
    pub chapter_name: String,
    pub novel_title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum Progress {
    Queued { chapter_id: i64 },
    Downloading { chapter_id: i64 },
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
    dir: PathBuf,
    inner: Mutex<Inner>,
    wake: Condvar,
}

impl Downloads {
    pub fn new(dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir, inner: Mutex::new(Inner::default()), wake: Condvar::new() })
    }

    pub fn path(&self, novel_id: i64, chapter_id: i64) -> PathBuf {
        self.dir.join(novel_id.to_string()).join(format!("{chapter_id}.txt"))
    }

    pub fn read(&self, novel_id: i64, chapter_id: i64) -> Option<String> {
        std::fs::read_to_string(self.path(novel_id, chapter_id)).ok()
    }

    /// Chapter ids of a novel that have a saved copy.
    pub fn downloaded(&self, novel_id: i64) -> Vec<i64> {
        let Ok(entries) = std::fs::read_dir(self.dir.join(novel_id.to_string())) else { return vec![] };
        entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                name.strip_suffix(".txt")?.parse().ok()
            })
            .collect()
    }

    pub fn delete(&self, novel_id: i64, chapter_ids: &[i64]) -> Result<()> {
        for id in chapter_ids {
            let p = self.path(novel_id, *id);
            if p.exists() {
                std::fs::remove_file(&p).with_context(|| format!("deleting {}", p.display()))?;
            }
        }
        Ok(())
    }

    pub fn delete_novel(&self, novel_id: i64) -> Result<()> {
        let d = self.dir.join(novel_id.to_string());
        if d.exists() {
            std::fs::remove_dir_all(d)?;
        }
        Ok(())
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
        walk(&self.dir)
    }

    pub fn enqueue(&self, items: Vec<QueueItem>, app: &AppHandle) {
        let mut inner = self.inner.lock().unwrap();
        for it in items {
            let dup = inner.queue.iter().any(|q| q.chapter_id == it.chapter_id)
                || inner.active.as_ref().is_some_and(|a| a.chapter_id == it.chapter_id)
                || self.path(it.novel_id, it.chapter_id).exists();
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
                    let result = fetch_and_save(&state, &item);
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

fn fetch_and_save(state: &AppState, item: &QueueItem) -> Result<()> {
    let chapter = state.db.chapter(item.chapter_id)?;
    let novel = state.db.novel(chapter.novel_id)?;
    let path = state.plugins.path_for(&novel.source_id)?;
    let text = plugins::content(&path, &chapter.url)?;
    anyhow::ensure!(!text.trim().is_empty(), "the source returned an empty chapter");
    let out = state.downloads.path(novel.id, chapter.id);
    std::fs::create_dir_all(out.parent().unwrap())?;
    // Write then rename, so a half-written file never counts as downloaded.
    let tmp = out.with_extension("part");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &out)?;
    Ok(())
}
