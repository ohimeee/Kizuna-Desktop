//! Tauri commands: the UI's API. Every command runs its work on a blocking thread — plugin
//! calls do synchronous network I/O and SQLite is synchronous too.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::{self, Category, Chapter, Db, HistoryItem, LibraryItem, Novel, UpdateItem};
use crate::downloads::{Downloads, QueueItem};
use crate::plugins::{self, IndexEntry, NovelPage, PluginStore, SourceInfo};

pub struct AppState {
    pub db: Db,
    pub plugins: PluginStore,
    pub downloads: Downloads,
    pub manga: manga::ExtHost,
    pub manga_tmp: std::path::PathBuf,
    pub image_cache: std::path::PathBuf,
}

pub type Shared<'a> = State<'a, Arc<AppState>>;

type CmdResult<T> = Result<T, String>;

async fn blocking<T, F>(state: &Shared<'_>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> anyhow::Result<T> + Send + 'static,
{
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || f(&state))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

// ---- sources -----------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSource {
    #[serde(flatten)]
    info: Option<SourceInfo>,
    file: String,
    error: Option<String>,
}

#[tauri::command]
pub async fn sources_installed(state: Shared<'_>) -> CmdResult<Vec<InstalledSource>> {
    blocking(&state, |s| {
        Ok(s.plugins
            .list()
            .into_iter()
            .map(|(path, info)| {
                let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                match info {
                    Ok(i) => InstalledSource { info: Some(i), file, error: None },
                    Err(e) => InstalledSource { info: None, file, error: Some(format!("{e:#}")) },
                }
            })
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn sources_index(state: Shared<'_>, url: Option<String>) -> CmdResult<Vec<IndexEntry>> {
    blocking(&state, move |_| plugins::fetch_index(url.as_deref().unwrap_or(plugins::DEFAULT_INDEX_URL))).await
}

#[tauri::command]
pub async fn source_install(state: Shared<'_>, entry: IndexEntry) -> CmdResult<SourceInfo> {
    blocking(&state, move |s| s.plugins.install(&entry)).await
}

#[tauri::command]
pub async fn source_uninstall(state: Shared<'_>, source_id: String) -> CmdResult<()> {
    blocking(&state, move |s| s.plugins.uninstall(&source_id)).await
}

/// A source listing page, with each novel's local id and library flag attached.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowsePage {
    novels: Vec<BrowseItem>,
    has_next_page: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseItem {
    id: i64,
    title: String,
    url: String,
    cover: Option<String>,
    favorite: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum BrowseQuery {
    Popular,
    Latest,
    Search { query: String, filters: serde_json::Map<String, serde_json::Value> },
}

#[tauri::command]
pub async fn source_browse(state: Shared<'_>, source_id: String, query: BrowseQuery, page: i32) -> CmdResult<BrowsePage> {
    blocking(&state, move |s| {
        let path = s.plugins.path_for(&source_id)?;
        let result: NovelPage = match &query {
            BrowseQuery::Popular => plugins::popular(&path, page)?,
            BrowseQuery::Latest => plugins::latest(&path, page)?,
            BrowseQuery::Search { query, filters } => {
                plugins::search(&path, query, page, &serde_json::Value::Object(filters.clone()).to_string())?
            }
        };
        let urls: Vec<String> = result.novels.iter().map(|n| n.url.clone()).collect();
        let favorites = s.db.favorites_among(&source_id, &urls)?;
        let mut novels = Vec::with_capacity(result.novels.len());
        for n in result.novels {
            let id = s.db.upsert_listed_novel(&source_id, &n.url, &n.title, n.cover.as_deref())?;
            novels.push(BrowseItem { id, favorite: favorites.contains(&n.url), title: n.title, url: n.url, cover: n.cover });
        }
        Ok(BrowsePage { novels, has_next_page: result.has_next_page })
    })
    .await
}

// ---- novels ------------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelView {
    novel: Novel,
    chapters: Vec<Chapter>,
    category_ids: Vec<i64>,
    source: Option<SourceInfo>,
    /// Chapter ids with an offline copy.
    downloaded: Vec<i64>,
    /// Display name of the source (novel plugin or manga extension).
    source_name: Option<String>,
}

fn manga_source_name(s: &AppState, source_id: &str) -> Option<String> {
    s.db.settings().ok()?.get(MANGA_SOURCE_CACHE)?.get(source_id)?.get("name")?.as_str().map(str::to_string)
}

fn novel_view(s: &AppState, id: i64) -> anyhow::Result<NovelView> {
    let novel = s.db.novel(id)?;
    let source = if novel.kind == "manga" {
        None
    } else {
        s.plugins.path_for(&novel.source_id).ok().and_then(|p| plugins::load_info(&p).ok())
    };
    let source_name = if novel.kind == "manga" {
        manga_source_name(s, &novel.source_id)
    } else {
        source.as_ref().map(|i| i.name.clone())
    };
    Ok(NovelView {
        chapters: s.db.chapters(id)?,
        category_ids: s.db.novel_categories(id)?,
        downloaded: s.downloads.downloaded(id),
        novel,
        source,
        source_name,
    })
}

#[tauri::command]
pub async fn novel_get(state: Shared<'_>, id: i64) -> CmdResult<NovelView> {
    blocking(&state, move |s| novel_view(s, id)).await
}

/// Fetches details and/or the chapter list from the source, saves them, returns the result.
#[tauri::command]
pub async fn novel_refresh(state: Shared<'_>, id: i64, details: bool, chapters: bool) -> CmdResult<NovelView> {
    blocking(&state, move |s| {
        let novel = s.db.novel(id)?;
        if novel.kind == "manga" {
            manga_refresh(s, &novel, details, chapters)?;
            return novel_view(s, id);
        }
        let path = s.plugins.path_for(&novel.source_id)?;
        if details {
            let d = plugins::details(&path, &novel.url)?;
            s.db.apply_details(id, &d)?;
        }
        if chapters {
            let list = plugins::chapters(&path, &novel.url)?;
            s.db.sync_chapters(id, &list)?;
        }
        novel_view(s, id)
    })
    .await
}

#[tauri::command]
pub async fn novel_set_favorite(state: Shared<'_>, id: i64, favorite: bool, category_ids: Option<Vec<i64>>) -> CmdResult<()> {
    blocking(&state, move |s| {
        s.db.set_favorite(id, favorite)?;
        if let Some(c) = category_ids {
            s.db.set_novel_categories(id, &c)?;
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn novel_set_categories(state: Shared<'_>, id: i64, category_ids: Vec<i64>) -> CmdResult<()> {
    blocking(&state, move |s| s.db.set_novel_categories(id, &category_ids)).await
}

// ---- chapters / reader -------------------------------------------------------------------

#[tauri::command]
pub async fn chapter_get(state: Shared<'_>, id: i64) -> CmdResult<Chapter> {
    blocking(&state, move |s| s.db.chapter(id)).await
}

#[tauri::command]
pub async fn chapter_content(state: Shared<'_>, chapter_id: i64) -> CmdResult<String> {
    blocking(&state, move |s| {
        let chapter = s.db.chapter(chapter_id)?;
        if let Some(saved) = s.downloads.read(chapter.novel_id, chapter.id) {
            return Ok(saved);
        }
        let novel = s.db.novel(chapter.novel_id)?;
        let path = s.plugins.path_for(&novel.source_id)?;
        plugins::content(&path, &chapter.url)
    })
    .await
}

#[tauri::command]
pub async fn chapter_save_progress(
    state: Shared<'_>,
    chapter_id: i64,
    paragraph: i64,
    percent: i64,
    mark_read: bool,
) -> CmdResult<()> {
    blocking(&state, move |s| s.db.save_progress(chapter_id, paragraph, percent, mark_read)).await
}

#[tauri::command]
pub async fn chapters_set_read(state: Shared<'_>, ids: Vec<i64>, read: bool) -> CmdResult<()> {
    blocking(&state, move |s| s.db.set_read(&ids, read)).await
}

#[tauri::command]
pub async fn chapters_set_bookmark(state: Shared<'_>, ids: Vec<i64>, bookmark: bool) -> CmdResult<()> {
    blocking(&state, move |s| s.db.set_bookmark(&ids, bookmark)).await
}

// ---- library, categories, history, updates -----------------------------------------------

#[tauri::command]
pub async fn library_list(state: Shared<'_>) -> CmdResult<Vec<LibraryItem>> {
    blocking(&state, |s| s.db.library()).await
}

#[tauri::command]
pub async fn categories_list(state: Shared<'_>) -> CmdResult<Vec<Category>> {
    blocking(&state, |s| s.db.categories()).await
}

#[tauri::command]
pub async fn category_create(state: Shared<'_>, name: String) -> CmdResult<Category> {
    blocking(&state, move |s| s.db.create_category(&name)).await
}

#[tauri::command]
pub async fn category_rename(state: Shared<'_>, id: i64, name: String) -> CmdResult<()> {
    blocking(&state, move |s| s.db.rename_category(id, &name)).await
}

#[tauri::command]
pub async fn category_delete(state: Shared<'_>, id: i64) -> CmdResult<()> {
    blocking(&state, move |s| s.db.delete_category(id)).await
}

#[tauri::command]
pub async fn categories_reorder(state: Shared<'_>, ids: Vec<i64>) -> CmdResult<()> {
    blocking(&state, move |s| s.db.reorder_categories(&ids)).await
}

#[tauri::command]
pub async fn history_list(state: Shared<'_>) -> CmdResult<Vec<HistoryItem>> {
    blocking(&state, |s| s.db.history(500)).await
}

#[tauri::command]
pub async fn history_remove(state: Shared<'_>, novel_id: i64) -> CmdResult<()> {
    blocking(&state, move |s| s.db.remove_history(novel_id)).await
}

#[tauri::command]
pub async fn history_clear(state: Shared<'_>) -> CmdResult<()> {
    blocking(&state, |s| s.db.clear_history()).await
}

#[tauri::command]
pub async fn updates_list(state: Shared<'_>) -> CmdResult<Vec<UpdateItem>> {
    blocking(&state, |s| s.db.updates(500)).await
}

/// Checks every library novel for new chapters. Returns how many new chapters were found
/// and which novels failed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryUpdateResult {
    new_chapters: usize,
    checked: usize,
    errors: Vec<String>,
}

#[tauri::command]
pub async fn library_update(state: Shared<'_>) -> CmdResult<LibraryUpdateResult> {
    blocking(&state, |s| {
        let mut res = LibraryUpdateResult { new_chapters: 0, checked: 0, errors: vec![] };
        for item in s.db.library()? {
            let n = item.novel;
            let outcome = (|| -> anyhow::Result<usize> {
                if n.kind == "manga" {
                    return manga_refresh(s, &n, false, true);
                }
                let path = s.plugins.path_for(&n.source_id)?;
                let list = plugins::chapters(&path, &n.url)?;
                s.db.sync_chapters(n.id, &list)
            })();
            res.checked += 1;
            match outcome {
                Ok(c) => res.new_chapters += c,
                Err(e) => res.errors.push(format!("{}: {e:#}", n.title)),
            }
        }
        Ok(res)
    })
    .await
}

// ---- downloads ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadState {
    active: Option<QueueItem>,
    queue: Vec<QueueItem>,
    paused: bool,
}

#[tauri::command]
pub async fn downloads_enqueue(state: Shared<'_>, app: tauri::AppHandle, chapter_ids: Vec<i64>) -> CmdResult<()> {
    blocking(&state, move |s| {
        let mut items = Vec::with_capacity(chapter_ids.len());
        let mut titles = std::collections::HashMap::new();
        for id in chapter_ids {
            let c = s.db.chapter(id)?;
            let title = match titles.get(&c.novel_id) {
                Some(t) => t,
                None => {
                    let t = s.db.novel(c.novel_id)?.title;
                    titles.entry(c.novel_id).or_insert(t)
                }
            }
            .clone();
            items.push(QueueItem { chapter_id: c.id, novel_id: c.novel_id, chapter_name: c.name, novel_title: title });
        }
        s.downloads.enqueue(items, &app);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn downloads_cancel(state: Shared<'_>, app: tauri::AppHandle, chapter_ids: Vec<i64>) -> CmdResult<()> {
    blocking(&state, move |s| {
        s.downloads.cancel(&chapter_ids, &app);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn downloads_clear(state: Shared<'_>, app: tauri::AppHandle) -> CmdResult<()> {
    blocking(&state, move |s| {
        s.downloads.clear(&app);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn downloads_set_paused(state: Shared<'_>, paused: bool) -> CmdResult<()> {
    blocking(&state, move |s| {
        s.downloads.set_paused(paused);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn downloads_state(state: Shared<'_>) -> CmdResult<DownloadState> {
    blocking(&state, |s| {
        let (active, queue, paused) = s.downloads.snapshot();
        Ok(DownloadState { active, queue, paused })
    })
    .await
}

#[tauri::command]
pub async fn downloads_delete(state: Shared<'_>, novel_id: i64, chapter_ids: Vec<i64>) -> CmdResult<()> {
    blocking(&state, move |s| s.downloads.delete(novel_id, &chapter_ids)).await
}

#[tauri::command]
pub async fn downloads_delete_novel(state: Shared<'_>, novel_id: i64) -> CmdResult<()> {
    blocking(&state, move |s| s.downloads.delete_novel(novel_id)).await
}

#[tauri::command]
pub async fn downloads_size(state: Shared<'_>) -> CmdResult<u64> {
    blocking(&state, |s| Ok(s.downloads.size())).await
}

// ---- settings ----------------------------------------------------------------------------

#[tauri::command]
pub async fn settings_get(state: Shared<'_>) -> CmdResult<serde_json::Map<String, serde_json::Value>> {
    blocking(&state, |s| s.db.settings()).await
}

#[tauri::command]
pub async fn settings_set(state: Shared<'_>, key: String, value: serde_json::Value) -> CmdResult<()> {
    blocking(&state, move |s| s.db.set_setting(&key, &value)).await
}

#[tauri::command]
pub fn now_ms() -> i64 {
    db::now_ms()
}

// ---- manga (extension host) ----------------------------------------------------------------

use crate::manga::{self, InstalledExtension, RepoExtension};

/// Source names/langs of installed manga extensions, cached in settings so screens can show
/// them without starting the engine.
pub const MANGA_SOURCE_CACHE: &str = "_mangaSources";

fn cache_manga_sources(s: &AppState, exts: &[InstalledExtension]) -> anyhow::Result<()> {
    let map: serde_json::Map<String, serde_json::Value> = exts
        .iter()
        .flat_map(|e| e.sources.iter())
        .map(|src| {
            (
                src.id.clone(),
                serde_json::json!({ "name": src.name, "lang": src.lang, "pkgName": src.pkg_name,
                                    "supportsLatest": src.supports_latest, "baseUrl": src.base_url }),
            )
        })
        .collect();
    s.db.set_setting(MANGA_SOURCE_CACHE, &serde_json::Value::Object(map))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaEngineStatus {
    installed: bool,
    running: bool,
    java: String,
    jar: String,
    /// Installed download's version (None when not installed or running from dev overrides).
    version: Option<u32>,
    /// Version this app build ships with; newer than `version` means "update available".
    latest: u32,
    /// Bytes to download.
    download_size: u64,
    /// Using KIZUNA_JAVA / KIZUNA_EXT_HOST_JAR instead of the download.
    dev: bool,
}

fn engine_status(s: &AppState) -> MangaEngineStatus {
    let p = s.manga.paths();
    MangaEngineStatus {
        installed: p.installed(),
        running: s.manga.is_running(),
        java: p.java.display().to_string(),
        jar: p.jar.display().to_string(),
        version: manga::engine::installed_version(&p.engine_dir),
        latest: manga::engine::ENGINE_VERSION,
        download_size: manga::engine::ENGINE_SIZE,
        dev: p.dev,
    }
}

#[tauri::command]
pub async fn manga_engine_status(state: Shared<'_>) -> CmdResult<MangaEngineStatus> {
    blocking(&state, |s| Ok(engine_status(s))).await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineProgress {
    done: u64,
    total: u64,
}

static ENGINE_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Downloads and installs (or updates) the manga engine, emitting `manga-engine-progress`.
#[tauri::command]
pub async fn manga_engine_install(state: Shared<'_>, app: tauri::AppHandle) -> CmdResult<MangaEngineStatus> {
    use std::sync::atomic::Ordering;
    use tauri::Emitter;
    blocking(&state, move |s| {
        anyhow::ensure!(!ENGINE_BUSY.swap(true, Ordering::SeqCst), "The manga engine is already being installed");
        let result = (|| {
            s.manga.stop();
            let dir = s.manga.paths().engine_dir;
            manga::engine::install(&dir, |done, total| {
                let _ = app.emit("manga-engine-progress", EngineProgress { done, total });
            })
        })();
        ENGINE_BUSY.store(false, Ordering::SeqCst);
        result?;
        Ok(engine_status(s))
    })
    .await
}

#[tauri::command]
pub async fn manga_engine_remove(state: Shared<'_>) -> CmdResult<MangaEngineStatus> {
    blocking(&state, |s| {
        anyhow::ensure!(!ENGINE_BUSY.load(std::sync::atomic::Ordering::SeqCst), "The manga engine is being installed");
        s.manga.stop();
        manga::engine::remove(&s.manga.paths().engine_dir)?;
        Ok(engine_status(s))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoListing {
    extensions: Vec<RepoExtension>,
    errors: Vec<String>,
}

#[tauri::command]
pub async fn manga_repo_list(state: Shared<'_>, repos: Vec<String>) -> CmdResult<RepoListing> {
    blocking(&state, move |_| {
        let mut out = RepoListing { extensions: vec![], errors: vec![] };
        for r in repos {
            match manga::fetch_repo(&r) {
                Ok(mut list) => out.extensions.append(&mut list),
                Err(e) => out.errors.push(format!("{r}: {e:#}")),
            }
        }
        Ok(out)
    })
    .await
}

#[tauri::command]
pub async fn manga_extensions(state: Shared<'_>) -> CmdResult<Vec<InstalledExtension>> {
    blocking(&state, |s| {
        let exts = manga::extensions(&s.manga)?;
        cache_manga_sources(s, &exts)?;
        Ok(exts)
    })
    .await
}

#[tauri::command]
pub async fn manga_extension_install(state: Shared<'_>, entry: RepoExtension) -> CmdResult<InstalledExtension> {
    blocking(&state, move |s| {
        let ext = manga::install(&s.manga, &entry, &s.manga_tmp)?;
        cache_manga_sources(s, &manga::extensions(&s.manga)?)?;
        Ok(ext)
    })
    .await
}

#[tauri::command]
pub async fn manga_extension_uninstall(state: Shared<'_>, pkg: String) -> CmdResult<()> {
    blocking(&state, move |s| {
        manga::uninstall(&s.manga, &pkg)?;
        cache_manga_sources(s, &manga::extensions(&s.manga)?)
    })
    .await
}

#[tauri::command]
pub async fn manga_browse(
    state: Shared<'_>,
    source_id: String,
    kind: String,
    page: i32,
    query: Option<String>,
) -> CmdResult<BrowsePage> {
    blocking(&state, move |s| {
        let res = manga::browse(&s.manga, &source_id, &kind, page, query.as_deref().unwrap_or(""))?;
        let urls: Vec<String> = res.mangas.iter().map(|m| m.url.clone()).collect();
        let favorites = s.db.favorites_among(&source_id, &urls)?;
        let mut novels = Vec::with_capacity(res.mangas.len());
        for m in res.mangas {
            let id = s.db.upsert_listed(&source_id, &m.url, &m.title, m.thumbnail_url.as_deref(), "manga")?;
            if let Some(memo) = manga::memo_text(&m.memo) {
                s.db.set_novel_memo(id, &memo)?;
            }
            novels.push(BrowseItem { id, favorite: favorites.contains(&m.url), title: m.title, url: m.url, cover: m.thumbnail_url });
        }
        Ok(BrowsePage { novels, has_next_page: res.has_next_page })
    })
    .await
}

/// Manga equivalent of a novel refresh: details and/or chapters through the host.
pub fn manga_refresh(s: &AppState, novel: &Novel, details: bool, chapters: bool) -> anyhow::Result<usize> {
    if details {
        let memo = s.db.novel_memo(novel.id)?;
        let d = manga::details(&s.manga, &novel.source_id, &novel.url, &novel.title, &memo)?;
        if let Some(memo) = manga::memo_text(&d.memo) {
            s.db.set_novel_memo(novel.id, &memo)?;
        }
        let nd = plugins::NovelDetails {
            title: Some(d.title).filter(|t| !t.is_empty()),
            cover: d.thumbnail_url,
            author: match (d.author, d.artist) {
                (Some(a), Some(b)) if a != b => Some(format!("{a}, {b}")),
                (a, b) => a.or(b),
            },
            description: d.description,
            genres: Some(d.genre).filter(|g| !g.is_empty()),
            status: manga::status_text(d.status).map(str::to_string),
        };
        s.db.apply_details(novel.id, &nd)?;
    }
    if chapters {
        let memo = s.db.novel_memo(novel.id)?;
        let list: Vec<plugins::ChapterItem> = manga::chapters(&s.manga, &novel.source_id, &novel.url, &memo)?
            .into_iter()
            .map(|c| plugins::ChapterItem {
                memo: manga::memo_text(&c.memo),
                name: c.name,
                url: c.url,
                chapter_number: c.chapter_number,
                date_upload: c.date_upload,
                locked: false,
            })
            .collect();
        return s.db.sync_chapters(novel.id, &list);
    }
    Ok(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaPageRef {
    index: i32,
    /// Direct image URL when the source already knows it; otherwise resolved from `page_url`.
    image: Option<String>,
    page_url: String,
}

#[tauri::command]
pub async fn manga_pages(state: Shared<'_>, chapter_id: i64) -> CmdResult<Vec<MangaPageRef>> {
    blocking(&state, move |s| {
        let chapter = s.db.chapter(chapter_id)?;
        let novel = s.db.novel(chapter.novel_id)?;
        let memo = s.db.chapter_memo(chapter_id)?;
        let pages = manga::pages(&s.manga, &novel.source_id, &chapter.url, &memo)?;
        Ok(pages
            .into_iter()
            .map(|p| MangaPageRef { index: p.index, image: p.image_url.filter(|u| !u.is_empty()), page_url: p.url })
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn manga_sources_cached(state: Shared<'_>) -> CmdResult<serde_json::Value> {
    blocking(&state, |s| Ok(s.db.settings()?.get(MANGA_SOURCE_CACHE).cloned().unwrap_or(serde_json::json!({})))).await
}
