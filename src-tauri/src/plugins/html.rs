//! Jsoup-compatible HTML selection for plugin scripts (`selectText`, `selectAttr`, ...).
//!
//! The Android app backs these helpers with Jsoup, so plugins are written against Jsoup's
//! behaviour. Two differences from `scraper` matter and are bridged here:
//!
//! - **Text extraction.** Jsoup's `text()` collapses whitespace and treats `<br>` and block
//!   elements as word breaks; `ownText()` only reads direct text children. `scraper` just
//!   concatenates raw text nodes, which glues words together across `<p>`/`<br>`.
//! - **`:contains(text)`.** A Jsoup-only pseudo-class (case-insensitive substring of the
//!   element's text). NovelPing's details page uses it, so it is emulated by selecting the part
//!   before it, filtering by text, then selecting the remainder inside each match.

use anyhow::{Context, Result, anyhow, bail};
use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node, Selector};

/// Block-level tags that Jsoup's `text()` separates with a space.
const BLOCK_TAGS: &[&str] = &[
    "address", "article", "aside", "blockquote", "dd", "div", "dl", "dt", "fieldset",
    "figcaption", "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header",
    "hr", "li", "main", "nav", "ol", "p", "pre", "section", "table", "tbody", "td", "tfoot",
    "th", "thead", "tr", "ul",
];

pub fn select_text(html: &str, selector: &str) -> Result<String> {
    let doc = Html::parse_document(html);
    Ok(select(&doc, selector)?.first().map(|e| text(*e)).unwrap_or_default())
}

pub fn select_own_text(html: &str, selector: &str) -> Result<String> {
    let doc = Html::parse_document(html);
    Ok(select(&doc, selector)?.first().map(|e| own_text(*e)).unwrap_or_default())
}

pub fn select_attr(html: &str, selector: &str, attr: &str) -> Result<String> {
    let doc = Html::parse_document(html);
    Ok(select(&doc, selector)?
        .first()
        .and_then(|e| e.value().attr(attr))
        .unwrap_or_default()
        .to_string())
}

pub fn select_html(html: &str, selector: &str) -> Result<String> {
    let doc = Html::parse_document(html);
    Ok(select(&doc, selector)?.first().map(|e| e.html()).unwrap_or_default())
}

pub fn select_all_text(html: &str, selector: &str) -> Result<Vec<String>> {
    let doc = Html::parse_document(html);
    Ok(select(&doc, selector)?.into_iter().map(text).collect())
}

pub fn select_all_attr(html: &str, selector: &str, attr: &str) -> Result<Vec<String>> {
    let doc = Html::parse_document(html);
    Ok(select(&doc, selector)?
        .into_iter()
        .map(|e| e.value().attr(attr).unwrap_or_default().to_string())
        .collect())
}

// ---- selection --------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Scope<'a> {
    Doc(&'a Html),
    El(ElementRef<'a>),
}

fn select<'a>(doc: &'a Html, selector: &str) -> Result<Vec<ElementRef<'a>>> {
    select_in(Scope::Doc(doc), selector)
}

fn select_in<'a>(scope: Scope<'a>, selector: &str) -> Result<Vec<ElementRef<'a>>> {
    let selector = selector.trim();
    let Some(start) = selector.find(":contains(") else {
        return plain_select(scope, selector);
    };

    let inner_start = start + ":contains(".len();
    let close = selector[inner_start..]
        .find(')')
        .map(|i| inner_start + i)
        .ok_or_else(|| anyhow!("unclosed :contains( in selector `{selector}`"))?;
    let needle = selector[inner_start..close]
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_lowercase();
    let prefix = &selector[..start];
    let rest = &selector[close + 1..];

    // `li:contains(x)` → candidates are `li`; `:contains(x)` or `div :contains(x)` → any element.
    let prefix = if prefix.is_empty() || prefix.ends_with(char::is_whitespace) || prefix.ends_with('>') {
        format!("{prefix}*")
    } else {
        prefix.to_string()
    };

    let matched: Vec<ElementRef<'a>> = select_in(scope, &prefix)?
        .into_iter()
        .filter(|e| text(*e).to_lowercase().contains(&needle))
        .collect();

    if rest.trim().is_empty() {
        return Ok(matched);
    }

    let mut out: Vec<ElementRef<'a>> = Vec::new();
    for el in matched {
        let found = if let Some(child_sel) = rest.trim_start().strip_prefix('>') {
            // Direct children only: select descendants, keep those whose parent is `el`.
            select_in(Scope::El(el), child_sel)?
                .into_iter()
                .filter(|c| c.parent().map(|p| p.id()) == Some(el.id()))
                .collect()
        } else if rest.starts_with(char::is_whitespace) {
            select_in(Scope::El(el), rest)?
        } else {
            bail!("unsupported selector after :contains(): `{rest}` in `{selector}`");
        };
        for f in found {
            if !out.iter().any(|o| o.id() == f.id()) {
                out.push(f);
            }
        }
    }
    Ok(out)
}

/// Jsoup accepts unquoted attribute values that strict CSS rejects, e.g. `a[href*=/author/]`.
/// Quote any unquoted value so those selectors parse the same way.
fn quote_attr_values(selector: &str) -> String {
    let mut out = String::with_capacity(selector.len() + 8);
    let mut rest = selector;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..=open]);
        rest = &rest[open + 1..];
        let Some(close) = rest.find(']') else { break };
        let inner = &rest[..close];
        match inner.find('=') {
            Some(eq) => {
                let (name_op, value) = inner.split_at(eq + 1);
                let v = value.trim();
                if v.is_empty() || v.starts_with('"') || v.starts_with('\'') {
                    out.push_str(inner);
                } else {
                    // Keep a trailing case flag (` i` / ` s`) outside the quotes.
                    let (val, flag) = match v.rsplit_once(' ') {
                        Some((val, f)) if f == "i" || f == "s" => (val.trim(), format!(" {f}")),
                        _ => (v, String::new()),
                    };
                    out.push_str(name_op);
                    out.push('"');
                    out.push_str(&val.replace('"', "\\\""));
                    out.push('"');
                    out.push_str(&flag);
                }
            }
            None => out.push_str(inner),
        }
        out.push(']');
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

fn plain_select<'a>(scope: Scope<'a>, selector: &str) -> Result<Vec<ElementRef<'a>>> {
    let selector = &quote_attr_values(selector);
    let parsed = Selector::parse(selector)
        .map_err(|e| anyhow!("{e:?}"))
        .with_context(|| format!("invalid CSS selector `{selector}`"))?;
    Ok(match scope {
        Scope::Doc(doc) => doc.select(&parsed).collect(),
        Scope::El(el) => el.select(&parsed).collect(),
    })
}

// ---- Jsoup-style text -------------------------------------------------------------------

/// Jsoup `Element.text()`: all descendant text, whitespace collapsed, `<br>`/blocks as spaces.
pub fn text(el: ElementRef) -> String {
    let mut out = String::new();
    append_text(*el, &mut out);
    out.trim().to_string()
}

/// Jsoup `Element.ownText()`: direct text children only (plus `<br>` as a space).
pub fn own_text(el: ElementRef) -> String {
    let mut out = String::new();
    for child in el.children() {
        match child.value() {
            Node::Text(t) => push_normalized(&mut out, t),
            Node::Element(e) if e.name() == "br" => push_space(&mut out),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn append_text(node: NodeRef<Node>, out: &mut String) {
    for child in node.children() {
        match child.value() {
            Node::Text(t) => push_normalized(out, t),
            Node::Element(e) => {
                let name = e.name();
                if name == "br" {
                    push_space(out);
                    continue;
                }
                if matches!(name, "script" | "style" | "template") {
                    continue;
                }
                let block = BLOCK_TAGS.contains(&name);
                if block {
                    push_space(out);
                }
                append_text(child, out);
                if block {
                    push_space(out);
                }
            }
            _ => {}
        }
    }
}

fn push_normalized(out: &mut String, s: &str) {
    for c in s.chars() {
        // Jsoup's isActuallyWhitespace also folds &nbsp; (U+00A0), as does char::is_whitespace.
        if c.is_whitespace() {
            push_space(out);
        } else {
            out.push(c);
        }
    }
}

fn push_space(out: &mut String) {
    if !out.is_empty() && !out.ends_with(' ') {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"<html><body>
        <ul class="info-meta">
          <li><h3>Author:</h3> <a href="/author/x">Jane  Doe</a></li>
          <li><h3>Status:</h3> <a href="/status/ongoing">Ongoing</a></li>
        </ul>
        <div class="content"><p>Hello<br>world</p><p>Second   para</p></div>
        <span id="own">own <b>bold</b> text</span>
    </body></html>"#;

    #[test]
    fn text_collapses_like_jsoup() {
        assert_eq!(select_text(DOC, "div.content").unwrap(), "Hello world Second para");
    }

    #[test]
    fn own_text_skips_children() {
        assert_eq!(select_own_text(DOC, "#own").unwrap(), "own text");
    }

    #[test]
    fn contains_pseudo() {
        assert_eq!(select_text(DOC, "ul.info-meta li:contains(Status) a").unwrap(), "Ongoing");
        assert_eq!(select_text(DOC, "ul.info-meta li:contains(status) > a").unwrap(), "Ongoing");
    }

    #[test]
    fn attrs_and_missing() {
        assert_eq!(select_attr(DOC, "a", "href").unwrap(), "/author/x");
        assert_eq!(select_attr(DOC, "a.none", "href").unwrap(), "");
        assert_eq!(select_all_attr(DOC, "a", "href").unwrap().len(), 2);
    }

    #[test]
    fn unquoted_attr_values_like_jsoup() {
        assert_eq!(select_text(DOC, "ul.info-meta li a[href*=/author/]").unwrap(), "Jane Doe");
        assert_eq!(quote_attr_values(r#"a[href="/x"]"#), r#"a[href="/x"]"#);
        assert_eq!(quote_attr_values("a[href]"), "a[href]");
    }

    #[test]
    fn last_child_and_not() {
        assert_eq!(select_text(DOC, "ul li:last-child a").unwrap(), "Ongoing");
        assert_eq!(select_all_text(DOC, "li:not(:last-child) a").unwrap(), vec!["Jane Doe"]);
    }
}
