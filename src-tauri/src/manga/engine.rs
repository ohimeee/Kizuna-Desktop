//! The optional manga engine download: a trimmed Java runtime + the extension host jar, built by
//! `scripts/build-manga-engine.ps1` and attached to the GitHub release that introduced it.
//! Installed into `<app data>/manga-engine/` (`runtime/`, `kizuna-ext-host.jar`, `version.txt`).
//!
//! The version, URL and checksum are pinned here, so an app build always gets the engine it was
//! tested with. When the engine changes: bump `ENGINE_VERSION`, run the script, paste its output
//! below, and attach the zip to that release.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};

pub const ENGINE_VERSION: u32 = 1;
pub const ENGINE_SIZE: u64 = 129_012_432;
const ENGINE_SHA256: &str = "06b489274ec79ed67c06766d1aaaef96fa47a63e379b1a5e92e711a247994207";
const ENGINE_URL: &str =
    "https://github.com/ohimeee/Kizuna-Desktop/releases/download/v0.2.0/kizuna-manga-engine-1.zip";

/// The installed engine's version, if any.
pub fn installed_version(engine_dir: &Path) -> Option<u32> {
    std::fs::read_to_string(engine_dir.join("version.txt")).ok()?.trim().parse().ok()
}

fn sibling(dir: &Path, suffix: &str) -> PathBuf {
    let mut name = dir.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    dir.with_file_name(name)
}

/// Downloads, verifies and installs the engine, replacing any older one. The host must be
/// stopped first (Windows won't replace files a running JVM has open). `progress(done, total)`
/// is called as bytes arrive.
pub fn install(engine_dir: &Path, mut progress: impl FnMut(u64, u64)) -> Result<()> {
    let zip_path = sibling(engine_dir, ".download.zip");
    let staging = sibling(engine_dir, ".new");
    let old = sibling(engine_dir, ".old");
    let result = (|| {
        download(&zip_path, &mut progress)?;
        let _ = std::fs::remove_dir_all(&staging);
        let file = std::fs::File::open(&zip_path)?;
        // `extract` rejects entries that would escape the target folder.
        zip::ZipArchive::new(file)
            .context("the download isn't a valid zip")?
            .extract(&staging)
            .context("unpacking the manga engine")?;
        ensure!(installed_version(&staging) == Some(ENGINE_VERSION), "the download has the wrong engine version");
        ensure!(staging.join("runtime/bin/java.exe").is_file(), "the download has no Java runtime");
        // Swap in: current -> .old, staging -> current, then drop .old.
        let _ = std::fs::remove_dir_all(&old);
        if engine_dir.exists() {
            std::fs::rename(engine_dir, &old).context("the old manga engine is still in use")?;
        }
        if let Err(e) = std::fs::rename(&staging, engine_dir) {
            let _ = std::fs::rename(&old, engine_dir);
            return Err(e).context("installing the manga engine");
        }
        let _ = std::fs::remove_dir_all(&old);
        Ok(())
    })();
    let _ = std::fs::remove_file(&zip_path);
    let _ = std::fs::remove_dir_all(&staging);
    result
}

fn download(out: &Path, progress: &mut impl FnMut(u64, u64)) -> Result<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Own client: the plugin client's timeouts are sized for web pages, not a 120 MB file.
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(None)
        .build()?;
    // KIZUNA_ENGINE_URL: test a freshly built zip before it's on a release (same checksum rule).
    let url = std::env::var("KIZUNA_ENGINE_URL").unwrap_or_else(|_| ENGINE_URL.to_string());
    let mut response = client.get(&url).send().context("couldn't reach GitHub")?;
    ensure!(response.status().is_success(), "HTTP {} downloading the manga engine", response.status().as_u16());
    let total = response.content_length().unwrap_or(ENGINE_SIZE);
    let mut file = std::fs::File::create(out)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut last_report = 0u64;
    progress(0, total);
    loop {
        let n = response.read(&mut buf).context("the download was interrupted")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
        done += n as u64;
        if done - last_report >= 1024 * 1024 || done == total {
            progress(done, total);
            last_report = done;
        }
    }
    file.flush()?;
    progress(done, total);
    let hash = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>();
    if hash != ENGINE_SHA256 {
        bail!("the download is damaged (checksum mismatch); try again");
    }
    Ok(())
}

/// Deletes the engine. The host must be stopped first.
pub fn remove(engine_dir: &Path) -> Result<()> {
    if engine_dir.exists() {
        std::fs::remove_dir_all(engine_dir).context("the manga engine is still in use")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_version_and_names_siblings() {
        let dir = std::env::temp_dir().join(format!("kizuna-engine-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(installed_version(&dir), None);
        std::fs::write(dir.join("version.txt"), "1").unwrap();
        assert_eq!(installed_version(&dir), Some(1));
        assert_eq!(sibling(&dir, ".new").file_name().unwrap().to_string_lossy(), format!("kizuna-engine-test-{}.new", std::process::id()));
        remove(&dir).unwrap();
        assert!(!dir.exists());
    }
}
