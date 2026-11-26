//! Manga: Mihon extensions run by the extension host, with library data in Kizuna's own SQLite
//! (novels table, `kind = 'manga'`), so novels and manga share one library.

pub mod host;
pub mod images;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::plugins::http::HTTP;

pub use host::{EnginePaths, ExtHost};

// ---- extension repos (Mihon index.min.json) ----------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSource {
    pub name: String,
    pub lang: String,
    /// String: 64-bit ids lose precision as JS numbers.
    #[serde(deserialize_with = "id_to_string")]
    pub id: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoExtension {
    pub name: String,
    pub pkg: String,
    pub apk: String,
    pub lang: String,
    pub code: i64,
    pub version: String,
    #[serde(default, deserialize_with = "int_to_bool")]
    pub nsfw: bool,
    #[serde(default)]
    pub sources: Vec<RepoSource>,
    /// Filled in by us: which repo this came from, and its icon URL.
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub icon_url: String,
}

fn id_to_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    })
}

fn int_to_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Bool(b) => b,
        serde_json::Value::Number(n) => n.as_i64() == Some(1),
        _ => false,
    })
}

/// `https://host/path/index.min.json` → `https://host/path`
pub fn repo_base(url: &str) -> String {
    let u = url.trim().trim_end_matches('/');
    match u.rsplit_once('/') {
        Some((base, last)) if last.ends_with(".json") => base.to_string(),
        _ => u.to_string(),
    }
}

/// Normalises what the user pasted: a repo base or its index URL → the index URL.
pub fn repo_index_url(url: &str) -> String {
    let u = url.trim().trim_end_matches('/');
    if u.ends_with(".json") { u.to_string() } else { format!("{u}/index.min.json") }
}

pub fn fetch_repo(url: &str) -> Result<Vec<RepoExtension>> {
    let index = repo_index_url(url);
    let body = HTTP.get(&index, "")?;
    let mut list: Vec<RepoExtension> =
        serde_json::from_str(&body).with_context(|| format!("{index} isn't a Mihon extension repo index"))?;
    let base = repo_base(&index);
    for e in &mut list {
        e.icon_url = format!("{base}/icon/{}.png", e.pkg);
        e.repo = index.clone();
    }
    Ok(list)
}

// ---- host-side types ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSource {
    pub id: String,
    pub name: String,
    pub lang: String,
    pub supports_latest: bool,
    pub base_url: Option<String>,
    pub pkg_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledExtension {
    pub pkg_name: String,
    pub name: String,
    pub version_name: String,
    pub version_code: i64,
    pub lang: String,
    pub nsfw: bool,
    pub signature_hash: Option<String>,
    pub sources: Vec<InstalledSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaDto {
    pub url: String,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub author: Option<String>,
    pub artist: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub genre: Vec<String>,
    #[serde(default)]
    pub status: i32,
    #[serde(default)]
    pub initialized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaPage {
    pub mangas: Vec<MangaDto>,
    pub has_next_page: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDto {
    pub url: String,
    pub name: String,
    pub date_upload: i64,
    pub chapter_number: f64,
    pub scanlator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageDto {
    pub index: i32,
    pub url: String,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDto {
    pub path: String,
    pub content_type: Option<String>,
}

/// Mihon's SManga status ints → the words the shared UI shows.
pub fn status_text(s: i32) -> Option<&'static str> {
    match s {
        1 => Some("Ongoing"),
        2 => Some("Completed"),
        3 => Some("Licensed"),
        4 => Some("Publishing finished"),
        5 => Some("Cancelled"),
        6 => Some("On hiatus"),
        _ => None,
    }
}

// ---- operations --------------------------------------------------------------------------

pub fn extensions(host: &ExtHost) -> Result<Vec<InstalledExtension>> {
    host.call("extensions", json!({}))
}

/// Downloads an extension APK from its repo and installs it through the host.
pub fn install(host: &ExtHost, ext: &RepoExtension, tmp_dir: &Path) -> Result<InstalledExtension> {
    let url = format!("{}/apk/{}", repo_base(&ext.repo), ext.apk);
    std::fs::create_dir_all(tmp_dir)?;
    let apk: PathBuf = tmp_dir.join(format!("{}.apk", ext.pkg));
    HTTP.download(&url, &apk).with_context(|| format!("downloading {}", ext.name))?;
    let result = host.call("install", json!({ "apk": apk }));
    let _ = std::fs::remove_file(&apk);
    result
}

pub fn uninstall(host: &ExtHost, pkg: &str) -> Result<()> {
    host.call::<serde_json::Value>("uninstall", json!({ "pkg": pkg })).map(|_| ())
}

pub fn browse(host: &ExtHost, source: &str, kind: &str, page: i32, query: &str) -> Result<MangaPage> {
    match kind {
        "popular" => host.call("popular", json!({ "source": source, "page": page })),
        "latest" => host.call("latest", json!({ "source": source, "page": page })),
        "search" => host.call("search", json!({ "source": source, "page": page, "query": query })),
        other => Err(anyhow!("unknown listing {other}")),
    }
}

pub fn details(host: &ExtHost, source: &str, url: &str, title: &str) -> Result<MangaDto> {
    host.call("details", json!({ "source": source, "url": url, "title": title }))
}

/// Chapters, returned newest-first by extensions (Mihon convention); reversed to reading order.
pub fn chapters(host: &ExtHost, source: &str, url: &str) -> Result<Vec<ChapterDto>> {
    let mut list: Vec<ChapterDto> = host.call("chapters", json!({ "source": source, "url": url }))?;
    list.reverse();
    Ok(list)
}

pub fn pages(host: &ExtHost, source: &str, chapter_url: &str) -> Result<Vec<PageDto>> {
    host.call("pages", json!({ "source": source, "url": chapter_url }))
}

pub fn image_url(host: &ExtHost, source: &str, page_url: &str) -> Result<String> {
    host.call("imageUrl", json!({ "source": source, "pageUrl": page_url }))
}

/// Fetches an image with the source's headers into `out` (the host writes the file).
pub fn image(host: &ExtHost, source: &str, url: &str, out: &Path) -> Result<ImageDto> {
    host.call("image", json!({ "source": source, "url": url, "out": out }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_urls() {
        assert_eq!(repo_index_url("https://x.dev/repo"), "https://x.dev/repo/index.min.json");
        assert_eq!(repo_index_url("https://x.dev/repo/index.min.json"), "https://x.dev/repo/index.min.json");
        assert_eq!(repo_base("https://x.dev/repo/index.min.json"), "https://x.dev/repo");
    }

    #[test]
    fn parses_mihon_index_entry() {
        let raw = r#"[{"name":"Tachiyomi: Example","pkg":"eu.kanade.tachiyomi.extension.en.example","apk":"tachiyomi-en.example-v1.4.1.apk","lang":"en","code":1,"version":"1.4.1","nsfw":0,"sources":[{"name":"Example","lang":"en","id":"1234567890123456789","baseUrl":"https://example.com"}]}]"#;
        let list: Vec<RepoExtension> = serde_json::from_str(raw).unwrap();
        assert_eq!(list[0].sources[0].id, "1234567890123456789");
        assert!(!list[0].nsfw);
    }
}
