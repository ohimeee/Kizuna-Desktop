//! Fetching through a real browser, for sites behind a Cloudflare check.
//!
//! The app never imitates a browser (see CLAUDE.md). Instead, a hidden WebView2 window (the same
//! engine and profile the UI runs in) makes the request with the page's own `fetch`, so the site
//! talks to a real browser with its own cookies. When Cloudflare wants a human check, the window
//! is shown so the user can solve it, then hidden again. The pass is kept in the WebView2
//! profile, shared with the main window, so covers from that site load there too.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use url::Url;

const LABEL: &str = "browser";
/// How long the user gets to solve a check.
const SOLVE_TIMEOUT: Duration = Duration::from_secs(180);
const LOAD_TIMEOUT: Duration = Duration::from_secs(45);
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);
/// The window shows as soon as the check asks for a click (`interactive`). Cloudflare's wording
/// changes now and then, so any check still up after this shows the window too.
const SHOW_AFTER: Duration = Duration::from_secs(8);

static APP: OnceLock<AppHandle> = OnceLock::new();
/// One browser request at a time (one window, one page).
static LOCK: Mutex<()> = Mutex::new(());
/// Hosts that answered the plain client with a Cloudflare check: later requests go straight
/// to the browser.
static HOSTS: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

pub fn init(app: AppHandle) {
    let _ = APP.set(app);
}

/// False in tests and command-line tools, which have no window to show.
pub fn available() -> bool {
    APP.get().is_some()
}

pub fn host_needs_browser(url: &str) -> bool {
    host_of(url).is_some_and(|h| HOSTS.lock().unwrap().contains(&h))
}

pub fn remember_host(url: &str) {
    if let Some(h) = host_of(url) {
        HOSTS.lock().unwrap().insert(h);
    }
}

fn host_of(url: &str) -> Option<String> {
    Url::parse(url).ok()?.host_str().map(str::to_string)
}

/// Is this response Cloudflare's "Just a moment..." check rather than the page?
pub fn is_challenge(status: u16, cf_mitigated: Option<&str>, body: &str) -> bool {
    cf_mitigated == Some("challenge")
        || (matches!(status, 403 | 503)
            && (body.contains("<title>Just a moment...</title>") || body.contains("/cdn-cgi/challenge-platform/")))
}

#[derive(Deserialize)]
struct PageState {
    href: String,
    ready: String,
    challenge: bool,
    /// The check is waiting for the user (its "complete the action below" prompt), not
    /// running its automatic test.
    #[serde(default)]
    interactive: bool,
    /// Set on the page before navigating away, so the old page isn't mistaken for the new one.
    old: bool,
}

#[derive(Deserialize)]
struct FetchResult {
    status: Option<u16>,
    cf: Option<String>,
    body: Option<String>,
    error: Option<String>,
}

const STATE_JS: &str = r#"JSON.stringify({
  href: location.href,
  ready: document.readyState,
  // Only Cloudflare's interstitial: a Turnstile widget inside a normal page (login or comment
  // forms) isn't a check the user has to pass.
  challenge: document.title === "Just a moment..." ||
    !!document.querySelector('#challenge-form, #challenge-running, #cf-challenge-running'),
  // The checkbox itself is in a cross-site frame; the page around it says what it's waiting
  // for: "Verify you are human by completing the action below" vs "Verifying you are human.
  // This may take a few seconds."
  interactive: (() => {
    const t = (document.body && document.body.innerText || "").toLowerCase();
    return /completing the action below|complete the action below|click to verify/.test(t) &&
      !/may take a few seconds/.test(t);
  })(),
  old: !!window.__kzOld
})"#;

/// Makes a request through the browser window and returns the response body.
pub fn request(url: &str, method: &str, body: Option<&str>, headers: &[(String, String)]) -> Result<String> {
    let app = APP.get().context("no browser window available")?;
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let target = Url::parse(url).with_context(|| format!("bad url {url}"))?;
    let origin = target.origin().ascii_serialization();
    let w = window(app)?;
    let result = request_in(&w, &target, &origin, url, method, body, headers);
    // Whatever happened, don't leave the window up once the request is over.
    let _ = w.hide();
    result
}

fn request_in(
    w: &WebviewWindow,
    target: &Url,
    origin: &str,
    url: &str,
    method: &str,
    body: Option<&str>,
    headers: &[(String, String)],
) -> Result<String> {
    let w = w.clone();

    // Be on the site first, so the page's fetch is same-origin and carries its cookies.
    let on_site = state(&w).is_ok_and(|s| Url::parse(&s.href).is_ok_and(|u| u.origin().ascii_serialization() == origin));
    if !on_site {
        open(&w, &format!("{origin}/"))?;
    }
    let r = page_fetch(&w, url, method, body, headers)?;
    if is_challenge(r.0, r.1.as_deref(), &r.2) {
        // The pass expired or this page wants its own check: load it for the user, then retry.
        open(&w, url)?;
        let r = page_fetch(&w, url, method, body, headers)?;
        if is_challenge(r.0, r.1.as_deref(), &r.2) {
            bail!("{} still shows a Cloudflare check", target.host_str().unwrap_or("the site"));
        }
        return Ok(r.2);
    }
    Ok(r.2)
}

fn window(app: &AppHandle) -> Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(LABEL) {
        return Ok(w);
    }
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External("about:blank".parse().unwrap()))
        .title("Verify you're human · Kizuna")
        .inner_size(520.0, 680.0)
        .visible(false)
        .build()
        .context("couldn't open the browser window")
}

/// Navigates and waits for the page, showing the window while Cloudflare checks the user.
fn open(w: &WebviewWindow, url: &str) -> Result<()> {
    let _ = eval(w, "window.__kzOld = true; true");
    w.navigate(url.parse().with_context(|| format!("bad url {url}"))?)?;
    let started = Instant::now();
    let mut shown = false;
    let mut challenged_at: Option<Instant> = None;
    let result = loop {
        let limit = if shown { SOLVE_TIMEOUT } else { LOAD_TIMEOUT };
        if started.elapsed() > limit {
            break Err(if shown {
                anyhow!("the Cloudflare check wasn't completed")
            } else {
                anyhow!("the page didn't load in time")
            });
        }
        // Errors are expected mid-navigation; the user closing the window ends the wait.
        match state(w) {
            Ok(s) if !s.old && s.challenge => {
                let since = *challenged_at.get_or_insert_with(Instant::now);
                if !shown && (s.interactive || since.elapsed() >= SHOW_AFTER) {
                    shown = true;
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
            Ok(s) if !s.old && s.ready == "complete" => break Ok(()),
            Ok(_) => {}
            Err(_) if w.is_visible().is_err() => break Err(anyhow!("the verification window was closed")),
            Err(_) => {}
        }
        std::thread::sleep(Duration::from_millis(300));
    };
    if shown {
        let _ = w.hide();
    }
    result
}

fn state(w: &WebviewWindow) -> Result<PageState> {
    let raw = eval(w, STATE_JS)?;
    Ok(serde_json::from_str(raw.as_str().context("no page state")?)?)
}

/// The page's own `fetch`: (status, cf-mitigated header, body).
fn page_fetch(w: &WebviewWindow, url: &str, method: &str, body: Option<&str>, headers: &[(String, String)]) -> Result<(u16, Option<String>, String)> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let opts = serde_json::json!({
        "method": method,
        "headers": headers.iter().cloned().collect::<std::collections::HashMap<_, _>>(),
        "body": body,
        "credentials": "include",
    });
    // ExecuteScript doesn't wait for promises: start the fetch, then poll for its result.
    let start = format!(
        "(() => {{ const k = window.__kz || (window.__kz = {{}}); const id = {id}; k[id] = null;
           fetch({url}, {opts})
             .then(async r => {{ k[id] = {{ status: r.status, cf: r.headers.get('cf-mitigated'), body: await r.text() }}; }})
             .catch(e => {{ k[id] = {{ error: String(e) }}; }});
           return true; }})()",
        url = serde_json::to_string(url)?,
    );
    eval(w, &start)?;
    let poll = format!("(() => {{ const k = window.__kz || {{}}; const v = k[{id}]; if (v) delete k[{id}]; return v ? JSON.stringify(v) : null; }})()");
    let started = Instant::now();
    loop {
        if let Some(raw) = eval(w, &poll)?.as_str() {
            let r: FetchResult = serde_json::from_str(raw)?;
            if let Some(e) = r.error {
                bail!("the browser couldn't fetch {url}: {e}");
            }
            return Ok((r.status.unwrap_or(0), r.cf, r.body.unwrap_or_default()));
        }
        if started.elapsed() > FETCH_TIMEOUT {
            bail!("the browser didn't get {url} in time");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Runs JavaScript in the browser window and returns its (JSON) result.
#[cfg(windows)]
fn eval(w: &WebviewWindow, js: &str) -> Result<serde_json::Value> {
    use webview2_com::{CoTaskMemPWSTR, ExecuteScriptCompletedHandler};

    let (tx, rx) = mpsc::channel::<Result<String, String>>();
    let js = js.to_string();
    w.with_webview(move |pw| unsafe {
        let fail = tx.clone();
        let run = || -> windows::core::Result<()> {
            let core = pw.controller().CoreWebView2()?;
            let handler = ExecuteScriptCompletedHandler::create(Box::new(move |hr, result| {
                let _ = tx.send(hr.map(|_| result).map_err(|e| e.to_string()));
                Ok(())
            }));
            let js = CoTaskMemPWSTR::from(js.as_str());
            core.ExecuteScript(*js.as_ref().as_pcwstr(), &handler)
        };
        if let Err(e) = run() {
            let _ = fail.send(Err(e.to_string()));
        }
    })?;
    let raw = rx
        .recv_timeout(Duration::from_secs(30))
        .map_err(|_| anyhow!("the browser window didn't answer"))?
        .map_err(|e| anyhow!(e))?;
    Ok(serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null))
}

#[cfg(not(windows))]
fn eval(_w: &WebviewWindow, _js: &str) -> Result<serde_json::Value> {
    bail!("browser fetching is only implemented on Windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spots_cloudflare_checks() {
        assert!(is_challenge(403, Some("challenge"), ""));
        assert!(is_challenge(403, None, "<html><head><title>Just a moment...</title>"));
        assert!(is_challenge(503, None, r#"<script src="/cdn-cgi/challenge-platform/h/b/orchestrate"#));
        assert!(!is_challenge(200, None, "<title>Just a moment...</title> in a novel"));
        assert!(!is_challenge(403, None, "Forbidden"));
    }

    #[test]
    fn remembers_hosts() {
        assert!(!host_needs_browser("https://cf.example/a"));
        remember_host("https://cf.example/x");
        assert!(host_needs_browser("https://cf.example/b"));
        assert!(!host_needs_browser("https://other.example/"));
    }
}
