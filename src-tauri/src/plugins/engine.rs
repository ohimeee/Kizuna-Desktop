//! Runs a novel plugin script in an embedded QuickJS engine — the desktop twin of the Android
//! app's `JsNovelSource`. Same contract (`docs/novel-sources/README.md` in the Kizuna repo):
//! the script calls `Register(json)` and assigns `globalThis.source`; we call one of its
//! functions and get back a JSON string (or plain text for `chapterContent`).
//!
//! Like Android, every call gets a fresh runtime: plugins are small, and this keeps calls
//! isolated and lets them run on any blocking thread in parallel.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use rquickjs::{CatchResultExt, Context, Ctx, Exception, Function, Object, Runtime, Value};

use super::{html, http::HTTP};

/// A plugin call gets this long (including its network requests) before it is interrupted.
const CALL_TIMEOUT: Duration = Duration::from_secs(120);
const MEMORY_LIMIT: usize = 256 * 1024 * 1024;

/// Same prelude as Android's `PRELUDE_SCRIPT`, so plugins behave identically on both.
const PRELUDE: &str = r#"
function Register(json) { __register(json); }

function fetchApi(url, options) {
    options = options || {};
    var method = (options.method || 'GET').toUpperCase();
    var headers = JSON.stringify(options.headers || {});
    var bodyText = method === 'GET'
        ? Http.get(url, headers)
        : Http.post(url, options.body || '', headers);
    return {
        text: function () { return bodyText; },
        json: function () { return JSON.parse(bodyText); },
    };
}

function selectText(html, selector) { return Html.selectText(html, selector); }
function selectOwnText(html, selector) { return Html.selectOwnText(html, selector); }
function selectAttr(html, selector, attr) { return Html.selectAttr(html, selector, attr); }
function selectHtml(html, selector) { return Html.selectHtml(html, selector); }
function selectAllText(html, selector) { return Html.selectAllText(html, selector); }
function selectAllAttr(html, selector, attr) { return Html.selectAllAttr(html, selector, attr); }
"#;

/// One of the plugin's entry points.
#[derive(Debug, Clone)]
pub enum Call {
    Popular(i32),
    Latest(i32),
    Search { query: String, page: i32, filters_json: String },
    Details(String),
    Chapters(String),
    Content(String),
}

/// Evaluates the script and returns the raw JSON it passed to `Register(...)`.
pub fn metadata(script: &str, file_name: &str) -> Result<String> {
    with_engine(script, file_name, |_ctx, _source| Ok(()))
        .map(|(_, registered)| registered)
}

/// Evaluates the script, then calls one of `globalThis.source`'s functions.
pub fn invoke(script: &str, file_name: &str, call: &Call) -> Result<String> {
    with_engine(script, file_name, |ctx, source| {
        let (name, result): (&str, rquickjs::Result<Value>) = match call {
            Call::Popular(page) => ("popularNovels", call_fn(&source, "popularNovels", (*page,))),
            Call::Latest(page) => ("latestNovels", call_fn(&source, "latestNovels", (*page,))),
            Call::Search { query, page, filters_json } => (
                "searchNovels",
                call_fn(&source, "searchNovels", (query.as_str(), *page, filters_json.as_str())),
            ),
            Call::Details(url) => ("novelDetails", call_fn(&source, "novelDetails", (url.as_str(),))),
            Call::Chapters(url) => ("chapterList", call_fn(&source, "chapterList", (url.as_str(),))),
            Call::Content(url) => ("chapterContent", call_fn(&source, "chapterContent", (url.as_str(),))),
        };
        let value = result.catch(ctx).map_err(|e| anyhow!("{name}() failed: {e}"))?;
        to_text(ctx, value).map_err(|e| anyhow!("{name}() returned an unusable value: {e}"))
    })
    .map(|(out, _)| out)
}

fn call_fn<'js, A: rquickjs::function::IntoArgs<'js>>(
    source: &Object<'js>,
    name: &str,
    args: A,
) -> rquickjs::Result<Value<'js>> {
    let f: Function = source.get(name)?;
    f.call(args)
}

/// Strings pass through; anything else (a plugin returning an object) is JSON-stringified.
fn to_text<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> Result<String> {
    if let Some(s) = value.as_string() {
        return Ok(s.to_string()?);
    }
    match ctx.json_stringify(value)? {
        Some(s) => Ok(s.to_string()?),
        None => Ok(String::new()),
    }
}

fn with_engine<T>(
    script: &str,
    file_name: &str,
    block: impl for<'js> FnOnce(&Ctx<'js>, Object<'js>) -> Result<T>,
) -> Result<(T, String)> {
    let rt = Runtime::new()?;
    rt.set_memory_limit(MEMORY_LIMIT);
    let deadline = Instant::now() + CALL_TIMEOUT;
    rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
    let context = Context::full(&rt)?;

    let registered: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    context.with(|ctx| -> Result<(T, String)> {
        install_bridges(&ctx, registered.clone()).map_err(|e| anyhow!("bridge setup: {e}"))?;

        ctx.eval::<(), _>(PRELUDE)
            .catch(&ctx)
            .map_err(|e| anyhow!("prelude: {e}"))?;
        let mut opts = rquickjs::context::EvalOptions::default();
        opts.global = true;
        opts.strict = false;
        opts.filename = Some(file_name.to_string());
        ctx.eval_with_options::<(), _>(script, opts)
            .catch(&ctx)
            .map_err(|e| anyhow!("{file_name}: {e}"))?;

        let meta = registered
            .borrow()
            .clone()
            .ok_or_else(|| anyhow!("{file_name} never called Register(...)"))?;
        let source: Object = ctx
            .globals()
            .get("source")
            .catch(&ctx)
            .map_err(|_| anyhow!("{file_name} did not assign globalThis.source"))?;

        Ok((block(&ctx, source)?, meta))
    })
}

/// Throws a JS `Error` carrying the Rust error's full context chain.
fn throw(ctx: &Ctx<'_>, e: anyhow::Error) -> rquickjs::Error {
    Exception::throw_message(ctx, &format!("{e:#}"))
}

fn install_bridges<'js>(ctx: &Ctx<'js>, registered: Rc<RefCell<Option<String>>>) -> rquickjs::Result<()> {
    let globals = ctx.globals();

    globals.set(
        "__register",
        Function::new(ctx.clone(), move |json: String| {
            *registered.borrow_mut() = Some(json);
        })?,
    )?;

    let http = Object::new(ctx.clone())?;
    http.set(
        "get",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, url: String, headers: rquickjs::function::Opt<String>| {
            HTTP.get(&url, headers.0.as_deref().unwrap_or("")).map_err(|e| throw(&ctx, e))
        })?,
    )?;
    http.set(
        "post",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, url: String, body: String, headers: rquickjs::function::Opt<String>| {
                HTTP.post(&url, &body, headers.0.as_deref().unwrap_or("")).map_err(|e| throw(&ctx, e))
            },
        )?,
    )?;
    http.set(
        "getCookie",
        Function::new(ctx.clone(), |url: String, name: String| HTTP.get_cookie(&url, &name))?,
    )?;
    globals.set("Http", http)?;

    let h = Object::new(ctx.clone())?;
    h.set(
        "selectText",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, doc: String, sel: String| {
            html::select_text(&doc, &sel).map_err(|e| throw(&ctx, e))
        })?,
    )?;
    h.set(
        "selectOwnText",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, doc: String, sel: String| {
            html::select_own_text(&doc, &sel).map_err(|e| throw(&ctx, e))
        })?,
    )?;
    h.set(
        "selectAttr",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, doc: String, sel: String, attr: String| {
            html::select_attr(&doc, &sel, &attr).map_err(|e| throw(&ctx, e))
        })?,
    )?;
    h.set(
        "selectHtml",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, doc: String, sel: String| {
            html::select_html(&doc, &sel).map_err(|e| throw(&ctx, e))
        })?,
    )?;
    h.set(
        "selectAllText",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, doc: String, sel: String| {
            html::select_all_text(&doc, &sel).map_err(|e| throw(&ctx, e))
        })?,
    )?;
    h.set(
        "selectAllAttr",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, doc: String, sel: String, attr: String| {
            html::select_all_attr(&doc, &sel, &attr).map_err(|e| throw(&ctx, e))
        })?,
    )?;
    globals.set("Html", h)?;

    Ok(())
}
