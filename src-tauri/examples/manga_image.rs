//! Debug helper: fetch one image through an installed extension, the way the kimg route does,
//! and print the result or the error.
//!     cargo run --example manga_image -- <manga data dir> <source id> <image url>
//! The data dir is `<app data>/manga` (installed extensions live there). Needs KIZUNA_JAVA and
//! KIZUNA_EXT_HOST_JAR, or an engine in `<data dir>/../manga-engine`.
use kizuna_desktop_lib::manga::{self, EnginePaths, ExtHost};

fn main() {
    let mut a = std::env::args().skip(1);
    let data = std::path::PathBuf::from(a.next().expect("data dir"));
    let source = a.next().expect("source id");
    let url = a.next().expect("image url");
    let engine = data.parent().unwrap().join("manga-engine");
    let host = ExtHost::new(EnginePaths::resolve(&engine, data));
    let out = std::env::temp_dir().join("kizuna-manga-image-test");
    match manga::image(&host, &source, &url, &out) {
        Ok(img) => println!("ok: {} bytes, {:?}", std::fs::metadata(&img.path).map(|m| m.len()).unwrap_or(0), img.content_type),
        Err(e) => println!("ERROR {e:#}"),
    }
    let _ = std::fs::remove_file(out);
    host.stop();
}
