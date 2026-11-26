//! `kimg://` image route for manga covers and pages. The webview can't send each source's
//! required headers (Referer, cookies, custom image interceptors), so images are fetched by the
//! extension host with the source's own client, cached on disk, then served from here.
//!
//!   kimg://localhost/i?s=<source id>&u=<image url>    direct image
//!   kimg://localhost/i?s=<source id>&p=<page url>     image URL resolved from a reader page
//!
//! (On Windows the webview addresses it as http://kimg.localhost/...)

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use tauri::http::{Response, StatusCode, header};

use super::ExtHost;

pub fn handle(host: &ExtHost, cache_dir: &Path, downloads: &crate::downloads::Downloads, db: &crate::db::Db, uri: &str) -> Response<Vec<u8>> {
    match serve(host, cache_dir, downloads, db, uri) {
        Ok((bytes, mime)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, "max-age=604800")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(bytes)
            .unwrap(),
        Err(e) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(format!("{e:#}").into_bytes())
            .unwrap(),
    }
}

fn serve(host: &ExtHost, cache_dir: &Path, downloads: &crate::downloads::Downloads, db: &crate::db::Db, uri: &str) -> Result<(Vec<u8>, &'static str)> {
    let parsed = url::Url::parse(uri).with_context(|| format!("bad image url {uri}"))?;
    // A downloaded page: `?l=<chapter id>/<file>`, only ever a page of a downloaded chapter.
    if let Some((_, l)) = parsed.query_pairs().find(|(k, _)| k == "l") {
        let path = downloads.page_file(db, &l).ok_or_else(|| anyhow!("bad local page {l}"))?;
        let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let mime = sniff(&bytes);
        return Ok((bytes, mime));
    }
    let mut source = None;
    let mut image = None;
    let mut page = None;
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "s" => source = Some(v.into_owned()),
            "u" => image = Some(v.into_owned()),
            "p" => page = Some(v.into_owned()),
            _ => {}
        }
    }
    let source = source.ok_or_else(|| anyhow!("missing source"))?;
    let key = match (&image, &page) {
        (Some(u), _) => format!("{source}|{u}"),
        (None, Some(p)) => format!("{source}|page|{p}"),
        _ => anyhow::bail!("missing image or page url"),
    };
    let path: PathBuf = cache_dir.join(format!("{:x}", md5::compute(key.as_bytes())));

    if !path.is_file() {
        let url = match (image, page) {
            (Some(u), _) => u,
            (None, Some(p)) => super::image_url(host, &source, &p)?,
            _ => unreachable!(),
        };
        std::fs::create_dir_all(cache_dir)?;
        super::image(host, &source, &url, &path)?;
    }
    let bytes = std::fs::read(&path)?;
    let mime = sniff(&bytes);
    Ok((bytes, mime))
}

/// File extension for saved pages, from the file's magic bytes.
pub fn extension(b: &[u8]) -> &'static str {
    match sniff(b) {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/jxl" => "jxl",
        _ => "bin",
    }
}

/// Content type from the file's magic bytes (the cache doesn't keep headers).
fn sniff(b: &[u8]) -> &'static str {
    match b {
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'G', b'I', b'F', b'8', ..] => "image/gif",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        [_, _, _, _, b'f', b't', b'y', b'p', b'a', b'v', b'i', b'f', ..] => "image/avif",
        [0xFF, 0x0A, ..] | [0, 0, 0, 0x0C, b'J', b'X', b'L', b' ', ..] => "image/jxl",
        _ => "application/octet-stream",
    }
}

/// Bytes used by the image cache.
pub fn cache_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|it| it.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::sniff;

    #[test]
    fn sniffs_common_formats() {
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), "image/jpeg");
        assert_eq!(sniff(b"\x89PNG\r\n"), "image/png");
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8"), "image/webp");
    }
}
