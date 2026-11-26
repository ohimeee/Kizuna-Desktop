//! Debug helper: parse a phone backup the way "Import backup" does and print what's in it.
//!     cargo run --example backup_dump -- <file.tachibk>
fn main() {
    let path = std::env::args().nth(1).expect("backup file");
    let bytes = std::fs::read(&path).expect("read");
    println!("{} bytes, starts {:02x?}", bytes.len(), &bytes[..bytes.len().min(4)]);
    match kizuna_desktop_lib::backup::parse(&bytes) {
        Ok(b) => {
            println!("{} entries, {} categories, {} sources, repos {:?}", b.manga.len(), b.categories.len(), b.sources.len(), b.repos);
            for m in b.manga.iter().take(20) {
                println!("  [{}] {} fav={} chapters={} history={} cats={:?} cover={:?}", m.source, m.title, m.favorite, m.chapters.len(), m.history.len(), m.categories, m.thumbnail.as_deref().map(|t| &t[..t.len().min(50)]));
            }
        }
        Err(e) => return println!("ERROR {e:#}"),
    }
    // Import into a throwaway library, classifying sources like the app does.
    use kizuna_desktop_lib::{backup, db::Db, plugins};
    let b = backup::parse(&bytes).unwrap();
    let novel: std::collections::HashSet<String> = plugins::fetch_index(plugins::DEFAULT_INDEX_URL)
        .map(|i| i.iter().map(|e| plugins::generate_id(&e.id, &e.lang).to_string()).collect())
        .unwrap_or_default();
    println!("catalog: {} novel source ids", novel.len());
    let path = std::env::temp_dir().join(format!("kizuna-import-test-{}.db", std::process::id()));
    let db = Db::open(&path).unwrap();
    let kinds = backup::SourceKinds { novel, installed_novel: Default::default(), installed_manga: Default::default() };
    match backup::import(&db, &b, &kinds) {
        Ok((sum, repos)) => println!("IMPORT OK {sum:?} repos={repos:?}
library: {}", db.library().unwrap().len()),
        Err(e) => println!("IMPORT ERROR {e:#}"),
    }
    drop(db);
    let _ = std::fs::remove_file(path);
}
