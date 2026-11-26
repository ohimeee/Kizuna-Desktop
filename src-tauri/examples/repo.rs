//! Debug helper: fetch an extension repo the way the app does.
//!     cargo run --example repo -- <repo url>
fn main() {
    let url = std::env::args().nth(1).expect("repo url");
    match kizuna_desktop_lib::manga::fetch_repo(&url) {
        Ok(list) => {
            println!("{} extensions", list.len());
            for e in list.iter().take(3) {
                println!("  {} {} [{}] nsfw={} {} sources -> {}", e.name, e.version, e.lang, e.nsfw, e.sources.len(), e.apk_url);
            }
        }
        Err(e) => println!("ERROR {e:#}"),
    }
}
