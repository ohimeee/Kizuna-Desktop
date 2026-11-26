//! Debug helper: install the manga engine into a folder the way "Enable manga" does.
//!     cargo run --example engine_install -- <folder>
//! Set KIZUNA_ENGINE_URL to test a local build (e.g. served from dist/).
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("target folder"));
    let mut last = 0;
    let r = kizuna_desktop_lib::manga::engine::install(&dir, |done, total| {
        let pct = done * 100 / total.max(1);
        if pct >= last + 20 || done == total { println!("{pct}% ({done}/{total})"); last = pct; }
    });
    match r {
        Ok(()) => println!("installed version {:?}", kizuna_desktop_lib::manga::engine::installed_version(&dir)),
        Err(e) => println!("ERROR {e:#}"),
    }
}
