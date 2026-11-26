//! Debug helper: fetch a URL through the plugin HTTP client and print what came back.
//!     cargo run --example fetch -- <url> [headers-json]
fn main() {
    let mut a = std::env::args().skip(1);
    let url = a.next().expect("url");
    let headers = a.next().unwrap_or_default();
    match kizuna_desktop_lib::plugins::http::HTTP.get(&url, &headers) {
        Ok(body) => {
            let title = body.split("<title>").nth(1).and_then(|s| s.split("</title>").next()).unwrap_or("-");
            println!("{} bytes, title: {}", body.len(), title.trim());
            if std::env::var("FULL").is_ok() { println!("{body}"); } else { println!("{}", body.chars().take(600).collect::<String>()); }
        }
        Err(e) => println!("ERROR {e:#}"),
    }
}
