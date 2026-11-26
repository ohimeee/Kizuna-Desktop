//! The `Http` bridge plugins call (`Http.get`, `Http.post`, `Http.getCookie`, and `fetchApi` on
//! top of them). Blocking on purpose: the plugin contract is synchronous, and every plugin call
//! runs on a `spawn_blocking` thread, never on the async runtime or the UI thread.
//!
//! Behaviour mirrors `JsNovelSource` on Android: the body is returned whatever the HTTP status,
//! except rate-limit answers (429, or 503 with `Retry-After`), which are waited out and retried.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, RETRY_AFTER};
use reqwest_cookie_store::{CookieStore, CookieStoreMutex};
use url::Url;

/// A desktop Edge UA matching the WebView2 runtime the app's UI runs in.
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/158.0.0.0 Safari/537.36 Edg/158.0.0.0";

const MAX_RATE_LIMIT_RETRIES: u32 = 3;
const DEFAULT_RETRY_AFTER_SECS: u64 = 2;

pub struct HttpClient {
    client: Client,
    jar: Arc<CookieStoreMutex>,
}

pub static HTTP: LazyLock<HttpClient> = LazyLock::new(HttpClient::new);

impl HttpClient {
    fn new() -> Self {
        let jar = Arc::new(CookieStoreMutex::new(CookieStore::default()));
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .cookie_provider(jar.clone())
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("failed to build HTTP client");
        Self { client, jar }
    }

    pub fn get(&self, url: &str, headers_json: &str) -> Result<String> {
        let headers = parse_headers(headers_json)?;
        self.execute(|| self.client.get(url).headers(headers.clone()))
            .with_context(|| format!("GET {url}"))
    }

    pub fn post(&self, url: &str, body: &str, headers_json: &str) -> Result<String> {
        let headers = parse_headers(headers_json)?;
        let body = body.to_string();
        self.execute(|| self.client.post(url).headers(headers.clone()).body(body.clone()))
            .with_context(|| format!("POST {url}"))
    }

    /// Value of cookie `name` that would be sent to `url`, or "" (matches Android's contract).
    pub fn get_cookie(&self, url: &str, name: &str) -> String {
        let Ok(url) = Url::parse(url) else { return String::new() };
        let store = self.jar.lock().expect("cookie jar poisoned");
        store
            .matches(&url)
            .into_iter()
            .find(|c| c.name() == name)
            .map(|c| c.value().to_string())
            .unwrap_or_default()
    }

    fn execute(&self, build: impl Fn() -> RequestBuilder) -> Result<String> {
        let mut retries = 0;
        loop {
            let response = build().send()?;
            let status = response.status().as_u16();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            let rate_limited = status == 429 || (status == 503 && retry_after.is_some());
            if !rate_limited || retries >= MAX_RATE_LIMIT_RETRIES {
                return Ok(response.text()?);
            }
            drop(response);
            let secs = retry_after
                .and_then(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(DEFAULT_RETRY_AFTER_SECS)
                .clamp(1, 10);
            retries += 1;
            std::thread::sleep(Duration::from_secs(secs));
        }
    }
}

fn parse_headers(headers_json: &str) -> Result<HeaderMap> {
    let mut map = HeaderMap::new();
    if headers_json.trim().is_empty() {
        return Ok(map);
    }
    let raw: HashMap<String, serde_json::Value> =
        serde_json::from_str(headers_json).context("headers must be a JSON object")?;
    for (k, v) in raw {
        let value = match v {
            serde_json::Value::String(s) => s,
            serde_json::Value::Null => continue,
            other => other.to_string(),
        };
        map.insert(HeaderName::from_bytes(k.as_bytes())?, HeaderValue::from_str(&value)?);
    }
    Ok(map)
}
