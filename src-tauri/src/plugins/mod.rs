//! Novel sources: JS plugin files, installed from the same index the Android app uses.

pub mod engine;
pub mod html;
pub mod http;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

pub use engine::Call;

/// The plugin repository the Android app installs from.
pub const DEFAULT_INDEX_URL: &str =
    "https://raw.githubusercontent.com/ohimeee/Kizuna/main/novel-sources/index.json";

// ---- DTOs (same shapes and defaults as Android's JsNovelSource) -------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterOption {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterDef {
    pub id: String,
    pub name: String,
    pub options: Vec<FilterOption>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataDto {
    id: String,
    name: String,
    lang: String,
    base_url: String,
    #[serde(default = "default_version")]
    version: String,
    #[serde(default = "default_true")]
    supports_latest: bool,
    #[serde(default)]
    filters: Vec<FilterDef>,
    #[serde(default)]
    icon_url: Option<String>,
}

fn default_version() -> String {
    "1.0.0".into()
}
fn default_true() -> bool {
    true
}

/// An installed source, as the UI sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    /// Stable numeric id, identical to the Android app's for the same plugin.
    /// Sent as a string because it can exceed JS's safe integer range.
    pub id: String,
    pub plugin_id: String,
    pub name: String,
    pub lang: String,
    pub base_url: String,
    pub version: String,
    pub supports_latest: bool,
    pub filters: Vec<FilterDef>,
    pub icon_url: Option<String>,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelItem {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelPage {
    pub novels: Vec<NovelItem>,
    #[serde(default)]
    pub has_next_page: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelDetails {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub genres: Option<Vec<String>>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterItem {
    pub name: String,
    pub url: String,
    #[serde(default = "default_chapter_number")]
    pub chapter_number: f64,
    #[serde(default)]
    pub date_upload: i64,
    #[serde(default)]
    pub locked: bool,
}

fn default_chapter_number() -> f64 {
    -1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    pub id: String,
    pub name: String,
    pub lang: String,
    pub version: String,
    pub file_url: String,
    #[serde(default)]
    pub icon_url: Option<String>,
}

// ---- ids ---------------------------------------------------------------------------------

/// Mirrors Mihon's `HttpSource.generateId`: first 8 bytes of md5("<id lowercase>/<lang>"),
/// big-endian, sign bit cleared. Must stay identical to Android so backups and library
/// entries map to the same source.
pub fn generate_id(plugin_id: &str, lang: &str) -> i64 {
    let digest = md5::compute(format!("{}/{}", plugin_id.to_lowercase(), lang));
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest.0[..8]);
    (u64::from_be_bytes(bytes) & i64::MAX as u64) as i64
}

// ---- loading and calling -----------------------------------------------------------------

pub fn load_info(path: &Path) -> Result<SourceInfo> {
    let script = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let file = file_name(path);
    let raw = engine::metadata(&script, &file)?;
    let meta: MetadataDto = serde_json::from_str(&raw).with_context(|| format!("{file}: bad Register(...) JSON"))?;
    Ok(SourceInfo {
        id: generate_id(&meta.id, &meta.lang).to_string(),
        plugin_id: meta.id,
        name: meta.name,
        lang: meta.lang,
        base_url: meta.base_url,
        version: meta.version,
        supports_latest: meta.supports_latest,
        filters: meta.filters,
        icon_url: meta.icon_url,
        file,
    })
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn run(path: &Path, call: &Call) -> Result<String> {
    let script = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    engine::invoke(&script, &file_name(path), call)
}

fn parse<T: for<'de> Deserialize<'de>>(what: &str, json: &str) -> Result<T> {
    serde_json::from_str(json).map_err(|e| {
        let preview: String = json.chars().take(200).collect();
        anyhow!("{what} returned invalid JSON ({e}): {preview}")
    })
}

pub fn popular(path: &Path, page: i32) -> Result<NovelPage> {
    parse("popularNovels", &run(path, &Call::Popular(page))?)
}

pub fn latest(path: &Path, page: i32) -> Result<NovelPage> {
    parse("latestNovels", &run(path, &Call::Latest(page))?)
}

pub fn search(path: &Path, query: &str, page: i32, filters_json: &str) -> Result<NovelPage> {
    let call = Call::Search {
        query: query.to_string(),
        page,
        filters_json: if filters_json.trim().is_empty() { "{}".into() } else { filters_json.to_string() },
    };
    parse("searchNovels", &run(path, &call)?)
}

pub fn details(path: &Path, url: &str) -> Result<NovelDetails> {
    parse("novelDetails", &run(path, &Call::Details(url.to_string()))?)
}

/// Chapters in natural reading order (chapter 1 first), as plugins return them.
pub fn chapters(path: &Path, url: &str) -> Result<Vec<ChapterItem>> {
    parse("chapterList", &run(path, &Call::Chapters(url.to_string()))?)
}

pub fn content(path: &Path, url: &str) -> Result<String> {
    run(path, &Call::Content(url.to_string()))
}

// ---- installed plugin store --------------------------------------------------------------

pub struct PluginStore {
    dir: PathBuf,
}

impl PluginStore {
    pub fn new(dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn list(&self) -> Vec<(PathBuf, Result<SourceInfo>)> {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "js") {
                    let info = load_info(&path);
                    out.push((path, info));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// Path of the installed plugin with this numeric source id.
    pub fn path_for(&self, source_id: &str) -> Result<PathBuf> {
        self.list()
            .into_iter()
            .find_map(|(p, info)| info.ok().filter(|i| i.id == source_id).map(|_| p))
            .ok_or_else(|| anyhow!("source {source_id} is not installed"))
    }

    pub fn install(&self, entry: &IndexEntry) -> Result<SourceInfo> {
        let script = http::HTTP.get(&entry.file_url, "")?;
        let safe_id: String = entry
            .id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let tmp = self.dir.join(format!("{safe_id}.js.part"));
        std::fs::write(&tmp, &script)?;
        // Validate before replacing a working copy.
        let check = load_info(&tmp);
        if let Err(e) = check {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.context(format!("downloaded {} is not a valid plugin", entry.name)));
        }
        let path = self.dir.join(format!("{safe_id}.js"));
        std::fs::rename(&tmp, &path)?;
        load_info(&path)
    }

    pub fn uninstall(&self, source_id: &str) -> Result<()> {
        let path = self.path_for(source_id)?;
        std::fs::remove_file(path)?;
        Ok(())
    }
}

pub fn fetch_index(url: &str) -> Result<Vec<IndexEntry>> {
    let body = http::HTTP.get(url, "")?;
    serde_json::from_str(&body).context("plugin index is not valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_matches_mihon_scheme() {
        // Deterministic, positive, and case-insensitive on the plugin id.
        let a = generate_id("royalroad", "en");
        assert!(a > 0);
        assert_eq!(a, generate_id("RoyalRoad", "en"));
        assert_ne!(a, generate_id("royalroad", "es"));
    }
}
