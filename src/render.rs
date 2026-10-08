//! Markdown fragment → HTML, with heading ids, link rewriting and Datalog highlighting.

use crate::version::Window;
use anyhow::{Result, bail};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
use std::collections::{BTreeSet, HashMap};

/// A link to another page or to a heading, checked after pages are assembled per version.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Link {
    /// Target page slug; `None` for a link within the same page.
    pub page: Option<String>,
    pub anchor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Rendered {
    pub html: String,
    /// Text of the fragment's first heading, if it starts with one.
    pub heading: Option<String>,
    pub ids: Vec<String>,
    pub links: BTreeSet<Link>,
}

pub enum LinkMode<'a> {
    /// Relative links must name a page slug.
    Authored,
    /// Relative links that are not page slugs point at the file in the source repository.
    Snapshot { blob_base: &'a str, dir: &'a str },
}

/// Allocates heading ids for one page, GitHub style: duplicates get `-1`, `-2`, …
/// Only headings whose fragments can appear in the same version count as duplicates, so a
/// heading rewritten for v3 keeps the same id as its v2 variant.
#[derive(Default)]
pub struct IdAllocator {
    seen: HashMap<String, Vec<Window>>,
    scope: Option<String>,
}

impl IdAllocator {
    fn alloc(&mut self, level: u8, text: &str, scoped: bool, window: Window) -> String {
        let base = slugify(text);
        let base = match (&self.scope, scoped && level >= 3) {
            (Some(scope), true) => format!("{scope}-{base}"),
            _ => base,
        };
        let seen = self.seen.entry(base.clone()).or_default();
        let n = seen
            .iter()
            .filter(|w| !w.intersect(window).is_empty())
            .count();
        let id = if n == 0 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        seen.push(window);
        if level <= 2 {
            self.scope = Some(id.clone());
        }
        id
    }
}

/// GitHub's heading-anchor algorithm.
pub fn slugify(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c)
            } else if c == ' ' {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

pub fn render(
    markdown: &str,
    slugs: &BTreeSet<String>,
    ids: &mut IdAllocator,
    window: Window,
    scoped_ids: bool,
    mode: &LinkMode<'_>,
) -> Result<Rendered> {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let events: Vec<Event> = Parser::new_ext(markdown, opts).collect();
    let mut out: Vec<Event> = Vec::with_capacity(events.len());
    let mut links = BTreeSet::new();
    let mut new_ids = Vec::new();
    let mut first_heading = None;
    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Heading { level, .. }) => {
                let level = *level as u8;
                let mut j = i + 1;
                let mut text = String::new();
                while !matches!(events[j], Event::End(TagEnd::Heading(_))) {
                    if let Event::Text(t) | Event::Code(t) = &events[j] {
                        text.push_str(t);
                    }
                    j += 1;
                }
                let id = ids.alloc(level, &text, scoped_ids, window);
                if i == 0 {
                    first_heading = Some(text.clone());
                }
                out.push(Event::Html(CowStr::from(format!("<h{level} id=\"{id}\">"))));
                for e in &events[i + 1..j] {
                    out.push(rewrite_link(e.clone(), slugs, mode, &mut links)?);
                }
                out.push(Event::Html(CowStr::from(format!(
                    "<a class=\"anchor\" href=\"#{id}\" aria-label=\"Link to this section\">#</a></h{level}>\n"
                ))));
                new_ids.push(id);
                i = j + 1;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => {
                        l.split_whitespace().next().unwrap_or("").to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                let mut j = i + 1;
                let mut code = String::new();
                while !matches!(events[j], Event::End(TagEnd::CodeBlock)) {
                    if let Event::Text(t) = &events[j] {
                        code.push_str(t);
                    }
                    j += 1;
                }
                let body = match lang.as_str() {
                    "datalog" | "clojure" | "edn" => highlight_datalog(&code),
                    _ => escape(&code),
                };
                let class = if lang.is_empty() {
                    String::new()
                } else {
                    format!(" class=\"language-{}\"", escape(&lang))
                };
                out.push(Event::Html(CowStr::from(format!(
                    "<pre><code{class}>{body}</code></pre>\n"
                ))));
                i = j + 1;
            }
            e => {
                out.push(rewrite_link(e.clone(), slugs, mode, &mut links)?);
                i += 1;
            }
        }
    }
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, out.into_iter());
    Ok(Rendered {
        html,
        heading: first_heading,
        ids: new_ids,
        links,
    })
}

fn rewrite_link<'a>(
    e: Event<'a>,
    slugs: &BTreeSet<String>,
    mode: &LinkMode<'_>,
    links: &mut BTreeSet<Link>,
) -> Result<Event<'a>> {
    let Event::Start(Tag::Link {
        link_type,
        dest_url,
        title,
        id,
    }) = e
    else {
        return Ok(e);
    };
    let dest = resolve_link(&dest_url, slugs, mode, links)?;
    Ok(Event::Start(Tag::Link {
        link_type,
        dest_url: CowStr::from(dest),
        title,
        id,
    }))
}

fn resolve_link(
    url: &str,
    slugs: &BTreeSet<String>,
    mode: &LinkMode<'_>,
    links: &mut BTreeSet<Link>,
) -> Result<String> {
    if url.contains("://") || url.starts_with("mailto:") || url.starts_with('/') {
        return Ok(url.to_string());
    }
    let (path, anchor) = match url.split_once('#') {
        Some((p, a)) => (p, Some(a.to_string())),
        None => (url, None),
    };
    if path.is_empty() {
        links.insert(Link { page: None, anchor });
        return Ok(url.to_string());
    }
    if slugs.contains(path) {
        links.insert(Link {
            page: Some(path.to_string()),
            anchor: anchor.clone(),
        });
        return Ok(match anchor {
            Some(a) => format!("../{path}/#{a}"),
            None => format!("../{path}/"),
        });
    }
    match mode {
        LinkMode::Authored => bail!("link `{url}` does not name a page"),
        LinkMode::Snapshot { blob_base, dir } => {
            let joined = normalize(&format!("{dir}{path}"));
            Ok(match anchor {
                Some(a) => format!("{blob_base}{joined}#{a}"),
                None => format!("{blob_base}{joined}"),
            })
        }
    }
}

fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

const DATALOG_HEADS: &[&str] = &[
    "query", "transact", "retract", "rule", "not", "not-join", "or", "or-join", "and",
];

/// Minimal Datalog/EDN highlighter: comments, strings, keywords, variables, numbers and
/// command heads.
pub fn highlight_datalog(code: &str) -> String {
    let chars: Vec<char> = code.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let span = |out: &mut String, class: &str, text: &str| {
        out.push_str(&format!(
            "<span class=\"tok-{class}\">{}</span>",
            escape(text)
        ));
    };
    let is_sym = |c: char| c.is_alphanumeric() || "-_/.?*+!<>=$'".contains(c);
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        if c == ';' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            span(
                &mut out,
                "comment",
                &chars[start..i].iter().collect::<String>(),
            );
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(chars.len());
            span(
                &mut out,
                "string",
                &chars[start..i].iter().collect::<String>(),
            );
        } else if c == ':' || c == '?' || c == '$' {
            i += 1;
            while i < chars.len() && is_sym(chars[i]) {
                i += 1;
            }
            let class = match c {
                ':' => "keyword",
                '?' => "var",
                _ => "slot",
            };
            span(&mut out, class, &chars[start..i].iter().collect::<String>());
        } else if c.is_ascii_digit()
            || (c == '-' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            span(
                &mut out,
                "number",
                &chars[start..i].iter().collect::<String>(),
            );
        } else if is_sym(c) {
            while i < chars.len() && is_sym(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let after_paren = out.ends_with('(');
            if after_paren && DATALOG_HEADS.contains(&word.as_str()) {
                span(&mut out, "head", &word);
            } else {
                out.push_str(&escape(&word));
            }
        } else {
            out.push_str(&escape(&c.to_string()));
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slugs() -> BTreeSet<String> {
        ["datalog-reference".to_string()].into_iter().collect()
    }

    #[test]
    fn github_slugs() {
        assert_eq!(
            slugify("File format v8 — goes live in v3.0.0"),
            "file-format-v8--goes-live-in-v300"
        );
        assert_eq!(slugify("`:as-of` — time travel"), "as-of--time-travel");
        assert_eq!(
            slugify("PRS-001 Unexpected end of input"),
            "prs-001-unexpected-end-of-input"
        );
    }

    #[test]
    fn heading_ids_and_duplicates() {
        let mut ids = IdAllocator::default();
        let r = render(
            "## Fixed\n\n## Fixed\n",
            &slugs(),
            &mut ids,
            Window::ALL,
            false,
            &LinkMode::Authored,
        )
        .unwrap();
        assert_eq!(r.ids, vec!["fixed", "fixed-1"]);
        assert_eq!(r.heading.as_deref(), Some("Fixed"));
        let mut ids = IdAllocator::default();
        let r = render(
            "## v2.0.4\n### Fixed\n## v2.0.3\n### Fixed\n",
            &slugs(),
            &mut ids,
            Window::ALL,
            true,
            &LinkMode::Authored,
        )
        .unwrap();
        assert_eq!(r.ids, vec!["v204", "v204-fixed", "v203", "v203-fixed"]);
    }

    #[test]
    fn variants_in_disjoint_windows_share_an_id() {
        let v3 = crate::version::Version::parse("v3.0.0").unwrap();
        let old = Window {
            from: None,
            to: Some(v3),
        };
        let new = Window {
            from: Some(v3),
            to: None,
        };
        let mut ids = IdAllocator::default();
        let a = render(
            "## Format\n",
            &slugs(),
            &mut ids,
            old,
            false,
            &LinkMode::Authored,
        )
        .unwrap();
        let b = render(
            "## Format\n",
            &slugs(),
            &mut ids,
            new,
            false,
            &LinkMode::Authored,
        )
        .unwrap();
        let c = render(
            "## Format\n",
            &slugs(),
            &mut ids,
            Window::ALL,
            false,
            &LinkMode::Authored,
        )
        .unwrap();
        assert_eq!(a.ids, vec!["format"]);
        assert_eq!(b.ids, vec!["format"]);
        assert_eq!(c.ids, vec!["format-2"]);
    }

    #[test]
    fn rewrites_page_links() {
        let mut ids = IdAllocator::default();
        let r = render(
            "[n](datalog-reference#negation) [x](#local) [e](https://example.com)",
            &slugs(),
            &mut ids,
            Window::ALL,
            false,
            &LinkMode::Authored,
        )
        .unwrap();
        assert!(r.html.contains("href=\"../datalog-reference/#negation\""));
        assert!(r.html.contains("href=\"#local\""));
        assert!(r.html.contains("href=\"https://example.com\""));
        assert_eq!(r.links.len(), 2);
        assert!(
            render(
                "[x](nope)",
                &slugs(),
                &mut ids,
                Window::ALL,
                false,
                &LinkMode::Authored
            )
            .is_err()
        );
    }

    #[test]
    fn snapshot_links_go_to_the_repo() {
        let mut ids = IdAllocator::default();
        let mode = LinkMode::Snapshot {
            blob_base: "https://github.com/o/r/blob/main/",
            dir: "docs/",
        };
        let r = render(
            "[p](../PHILOSOPHY.md#support)",
            &slugs(),
            &mut ids,
            Window::ALL,
            false,
            &mode,
        )
        .unwrap();
        assert!(
            r.html
                .contains("href=\"https://github.com/o/r/blob/main/PHILOSOPHY.md#support\"")
        );
    }

    #[test]
    fn highlights_datalog() {
        let h = highlight_datalog("(query [:find ?n :where [?e :name \"A<\"]]) ; hi");
        assert!(h.contains("<span class=\"tok-head\">query</span>"));
        assert!(h.contains("<span class=\"tok-var\">?n</span>"));
        assert!(h.contains("<span class=\"tok-keyword\">:find</span>"));
        assert!(h.contains("<span class=\"tok-string\">&quot;A&lt;&quot;</span>"));
        assert!(h.contains("<span class=\"tok-comment\">; hi</span>"));
    }
}
