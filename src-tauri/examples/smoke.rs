//! Runs every plugin in a folder end-to-end against the live sites:
//! Register → popular page 1 → details → chapter list → first chapter's text.
//!
//!     cargo run --example smoke -- D:\Coding\Kizuna\novel-sources [plugin-file-filter]

use std::path::PathBuf;
use std::time::Instant;

use kizuna_desktop_lib::plugins;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("usage: smoke <plugin-dir> [filter]"));
    let filter = args.next();

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("plugin dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "js"))
        .filter(|p| filter.as_ref().is_none_or(|f| p.to_string_lossy().contains(f.as_str())))
        .collect();
    files.sort();

    let mut failures = 0;
    for path in &files {
        let started = Instant::now();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        match check(path) {
            Ok(summary) => println!("PASS {name} ({:.1}s)\n{summary}", started.elapsed().as_secs_f32()),
            Err(e) => {
                failures += 1;
                println!("FAIL {name} ({:.1}s): {e:#}\n", started.elapsed().as_secs_f32());
            }
        }
    }
    println!("{} passed, {failures} failed", files.len() - failures);
    if failures > 0 {
        std::process::exit(1);
    }
}

fn check(path: &PathBuf) -> anyhow::Result<String> {
    let info = plugins::load_info(path)?;
    let mut s = format!("  {} [{}] v{} id={} filters={}\n", info.name, info.lang, info.version, info.id, info.filters.len());

    let page = plugins::popular(path, 1)?;
    anyhow::ensure!(!page.novels.is_empty(), "popularNovels(1) returned no novels");
    let first = &page.novels[0];
    s += &format!("  popular: {} novels, next={}, first=\"{}\"\n", page.novels.len(), page.has_next_page, first.title);

    let query: String = first.title.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    match plugins::search(path, &query, 1, "{}") {
        Ok(r) => s += &format!("  search \"{query}\": {} results\n", r.novels.len()),
        Err(e) => s += &format!("  search \"{query}\": ERROR {e:#}\n"),
    }

    let d = plugins::details(path, &first.url)?;
    s += &format!(
        "  details: author={:?} status={:?} genres={} desc={} chars\n",
        d.author,
        d.status,
        d.genres.as_ref().map_or(0, |g| g.len()),
        d.description.as_ref().map_or(0, |x| x.chars().count())
    );

    let chapters = plugins::chapters(path, &first.url)?;
    anyhow::ensure!(!chapters.is_empty(), "chapterList returned no chapters");
    let locked = chapters.iter().filter(|c| c.locked).count();
    s += &format!("  chapters: {} ({locked} locked), first=\"{}\"\n", chapters.len(), chapters[0].name);

    let body = plugins::content(path, &chapters[0].url)?;
    anyhow::ensure!(body.trim().len() > 200, "chapterContent is suspiciously short: {:?}", body);
    let preview: String = body.chars().filter(|c| !c.is_control()).take(90).collect();
    s += &format!("  content: {} chars: {preview}…\n", body.chars().count());
    Ok(s)
}
