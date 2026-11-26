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

// ---- extension repos ---------------------------------------------------------------------
//
// Two index formats exist:
// - v1 (legacy): `index.min.json` / `index.json`, an array of {name, pkg, apk, lang, code, ...};
//   APKs live at `<repo>/apk/<apk>`, icons at `<repo>/icon/<pkg>.png`.
// - v2 (Mihon 0.20.1+, e.g. Keiyoushi since 2026): `repo.json` points at `index.pb`; the same data
//   is published as JSON in `index.json` = {name, signingKey, extensionList: {extensions: [...]}},
//   each with direct `resources.apkUrl` / `iconUrl`. Repos on v2 leave a stub `index.min.json`
//   that only says "update your app", so v2 is tried first.

/// One extension from a repo, normalised across index formats. Sent to and back from the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoExtension {
    pub name: String,
    pub pkg: String,
    pub apk_url: String,
    pub icon_url: String,
    pub lang: String,
    pub code: i64,
    pub version: String,
    pub nsfw: bool,
    pub sources: Vec<RepoSource>,
    /// Index URL this came from.
    pub repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSource {
    /// String: 64-bit ids lose precision as JS numbers.
    pub id: String,
    pub name: String,
    pub lang: String,
    pub base_url: Option<String>,
}

#[derive(Deserialize)]
struct V1Ext {
    name: String,
    pkg: String,
    apk: String,
    lang: String,
    #[serde(deserialize_with = "num_or_str_i64")]
    code: i64,
    version: String,
    #[serde(default, deserialize_with = "int_to_bool")]
    nsfw: bool,
    #[serde(default)]
    sources: Vec<V1Source>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V1Source {
    name: String,
    lang: String,
    #[serde(deserialize_with = "id_to_string")]
    id: String,
    #[serde(default)]
    base_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V2Index {
    extension_list: V2List,
}

#[derive(Deserialize)]
struct V2List {
    extensions: Vec<V2Ext>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V2Ext {
    name: String,
    package_name: String,
    resources: V2Resources,
    #[serde(default, deserialize_with = "num_or_str_i64")]
    version_code: i64,
    version_name: String,
    #[serde(default)]
    content_warning: Option<String>,
    #[serde(default)]
    sources: Vec<V2Source>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V2Resources {
    apk_url: String,
    #[serde(default)]
    icon_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V2Source {
    #[serde(deserialize_with = "id_to_string")]
    id: String,
    name: String,
    language: String,
    #[serde(default)]
    home_url: Option<String>,
}

fn id_to_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    })
}

fn num_or_str_i64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(0),
        serde_json::Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
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

/// Whatever the user pasted (repo base, repo.json, index.pb, index.json, index.min.json) →
/// the repo's base URL.
pub fn repo_base(url: &str) -> String {
    let u = url.trim().trim_end_matches('/');
    match u.rsplit_once('/') {
        Some((base, last)) if last.ends_with(".json") || last.ends_with(".pb") => base.to_string(),
        _ => u.to_string(),
    }
}

fn shared_lang(langs: impl Iterator<Item = String>) -> String {
    let set: std::collections::BTreeSet<String> = langs.collect();
    if set.len() == 1 { set.into_iter().next().unwrap() } else { "all".into() }
}

fn from_v2(index: V2Index, repo: &str) -> Vec<RepoExtension> {
    index
        .extension_list
        .extensions
        .into_iter()
        .map(|e| RepoExtension {
            lang: shared_lang(e.sources.iter().map(|s| s.language.clone())),
            nsfw: e.content_warning.as_deref() == Some("CONTENT_WARNING_NSFW"),
            sources: e
                .sources
                .into_iter()
                .map(|s| RepoSource { id: s.id, name: s.name, lang: s.language, base_url: s.home_url })
                .collect(),
            name: e.name,
            pkg: e.package_name,
            apk_url: e.resources.apk_url,
            icon_url: e.resources.icon_url.unwrap_or_default(),
            code: e.version_code,
            version: e.version_name,
            repo: repo.to_string(),
        })
        .collect()
}

fn from_v1(list: Vec<V1Ext>, base: &str, repo: &str) -> Vec<RepoExtension> {
    list.into_iter()
        .map(|e| RepoExtension {
            apk_url: format!("{base}/apk/{}", e.apk),
            icon_url: format!("{base}/icon/{}.png", e.pkg),
            sources: e
                .sources
                .into_iter()
                .map(|s| RepoSource { id: s.id, name: s.name, lang: s.lang, base_url: s.base_url })
                .collect(),
            name: e.name,
            pkg: e.pkg,
            lang: e.lang,
            code: e.code,
            version: e.version,
            nsfw: e.nsfw,
            repo: repo.to_string(),
        })
        .collect()
}

/// Parses an index body in either format.
pub fn parse_index(body: &str, base: &str, repo: &str) -> Result<Vec<RepoExtension>> {
    let value: serde_json::Value = serde_json::from_str(body).context("not JSON")?;
    if value.get("extensionList").is_some() {
        let v2: V2Index = serde_json::from_value(value).context("unexpected v2 index format")?;
        return Ok(from_v2(v2, repo));
    }
    let v1: Vec<V1Ext> = serde_json::from_value(value).context("unexpected index format")?;
    Ok(from_v1(v1, base, repo))
}

pub fn fetch_repo(url: &str) -> Result<Vec<RepoExtension>> {
    let base = repo_base(url);
    let mut last_err = None;
    // index.json first: full v2 (or full v1) list. Then the legacy minified v1 list.
    for name in ["index.json", "index.min.json"] {
        let index = format!("{base}/{name}");
        match HTTP.get(&index, "").and_then(|body| parse_index(&body, &base, &index)) {
            Ok(list) if !list.is_empty() => return Ok(list),
            Ok(_) => last_err = Some(anyhow!("{index} has no extensions")),
            Err(e) => last_err = Some(e.context(format!("reading {index}"))),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("{base} isn't a Mihon extension repo")))
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
    let url = ext.apk_url.clone();
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
    fn repo_base_accepts_any_index_url() {
        for u in [
            "https://x.dev/repo",
            "https://x.dev/repo/",
            "https://x.dev/repo/index.min.json",
            "https://x.dev/repo/index.pb",
            "https://x.dev/repo/repo.json",
        ] {
            assert_eq!(repo_base(u), "https://x.dev/repo", "{u}");
        }
    }

    #[test]
    fn parses_v1_index() {
        let raw = r#"[{"name":"Tachiyomi: Example","pkg":"eu.kanade.tachiyomi.extension.en.example","apk":"tachiyomi-en.example-v1.4.1.apk","lang":"en","code":1,"version":"1.4.1","nsfw":0,"sources":[{"name":"Example","lang":"en","id":"1234567890123456789","baseUrl":"https://example.com"}]}]"#;
        let list = parse_index(raw, "https://x.dev/repo", "i").unwrap();
        assert_eq!(list[0].apk_url, "https://x.dev/repo/apk/tachiyomi-en.example-v1.4.1.apk");
        assert_eq!(list[0].sources[0].id, "1234567890123456789");
        assert!(!list[0].nsfw);
    }

    #[test]
    fn parses_v2_index() {
        let raw = r#"{"name":"Keiyoushi","extensionList":{"extensions":[{"name":"AchyManga","packageName":"eu.kanade.tachiyomi.extension.all.achymanga","resources":{"apkUrl":"https://e.x/a.apk","iconUrl":"https://e.x/i.png"},"extensionLib":"1.6","versionCode":"106001","versionName":"1.6.1","contentWarning":"CONTENT_WARNING_MIXED","sources":[{"id":"9124545289113350724","name":"AchyManga","language":"es","homeUrl":"https://achymanga.online"},{"id":"2311891545723109443","name":"AchyManga","language":"en"}]}]}}"#;
        let list = parse_index(raw, "https://x.dev/repo", "i").unwrap();
        let e = &list[0];
        assert_eq!((e.pkg.as_str(), e.apk_url.as_str(), e.code, e.lang.as_str()), ("eu.kanade.tachiyomi.extension.all.achymanga", "https://e.x/a.apk", 106001, "all"));
        assert!(!e.nsfw);
        assert_eq!(e.sources[1].base_url, None);
    }
}
