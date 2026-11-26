//! Debug helper: run the whole manga chain against a live extension, the way the app does
//! (repo -> install -> popular -> details -> chapters -> pages -> first image), with memos
//! stored in and read back from a throwaway library database.
//!     cargo run --example manga_chain -- <repo url> <extension name>
//! Needs KIZUNA_JAVA and KIZUNA_EXT_HOST_JAR (see CLAUDE.md).
use kizuna_desktop_lib::db::Db;
use kizuna_desktop_lib::manga::{self, EnginePaths, ExtHost};
use kizuna_desktop_lib::plugins::ChapterItem;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let repo = args.next().expect("repo url");
    let wanted = args.next().expect("extension name").to_lowercase();

    let work = std::env::temp_dir().join(format!("kizuna-manga-chain-{}", std::process::id()));
    std::fs::create_dir_all(&work)?;
    let host = ExtHost::new(EnginePaths::resolve(&work, work.join("data")));
    let db = Db::open(&work.join("library.db"))?;

    let list = manga::fetch_repo(&repo)?;
    let entry = list.iter().find(|e| e.name.to_lowercase().contains(&wanted)).expect("no such extension");
    println!("install {} {} via {}", entry.name, entry.version, if entry.jar_url.is_some() { "jar" } else { "apk" });
    let ext = manga::install(&host, entry, &work.join("tmp"))?;
    let source = &ext.sources[0].id;

    let popular = manga::browse(&host, source, "popular", 1, "")?;
    let m = popular.mangas.first().expect("popular is empty");
    println!("popular: {} results, first {:?} memo={}", popular.mangas.len(), m.title, m.memo);
    let id = db.upsert_listed(source, &m.url, &m.title, m.thumbnail_url.as_deref(), "manga")?;
    if let Some(memo) = manga::memo_text(&m.memo) {
        db.set_novel_memo(id, &memo)?;
    }

    let d = manga::details(&host, source, &m.url, &m.title, &db.novel_memo(id)?)?;
    println!("details: author={:?} status={} genres={} memo={}", d.author, d.status, d.genre.len(), d.memo);
    if let Some(memo) = manga::memo_text(&d.memo) {
        db.set_novel_memo(id, &memo)?;
    }

    let chapters = manga::chapters(&host, source, &m.url, &db.novel_memo(id)?)?;
    println!("chapters: {} (first {:?}, memo={})", chapters.len(), chapters.first().map(|c| &c.name), chapters.first().map(|c| c.memo.to_string()).unwrap_or_default());
    let items: Vec<ChapterItem> = chapters
        .iter()
        .map(|c| ChapterItem {
            name: c.name.clone(),
            url: c.url.clone(),
            chapter_number: c.chapter_number,
            date_upload: c.date_upload,
            locked: false,
            memo: manga::memo_text(&c.memo),
        })
        .collect();
    db.sync_chapters(id, &items)?;
    let first = db.chapters(id)?.into_iter().next().expect("no chapters");

    let pages = manga::pages(&host, source, &first.url, &db.chapter_memo(first.id)?)?;
    println!("pages: {} in {:?}", pages.len(), first.name);
    let p = pages.first().expect("no pages");
    let image = match p.image_url.clone().filter(|u| !u.is_empty()) {
        Some(u) => u,
        None => manga::image_url(&host, source, &p.url)?,
    };
    let out = work.join("page0");
    let img = manga::image(&host, source, &image, &out)?;
    println!("image: {} bytes, {:?} -> {}", std::fs::metadata(&img.path)?.len(), img.content_type, img.path);
    host.stop();
    Ok(())
}
