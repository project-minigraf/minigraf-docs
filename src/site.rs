//! HTML templates. Every page is complete HTML that works without JavaScript.

use crate::config::{Release, Site};
use crate::graph::{FragAt, NavEntry, frags_query};
use crate::render::escape;
use std::collections::HashMap;

pub struct PageCtx<'a> {
    pub site: &'a Site,
    pub releases: &'a [Release],
    /// The version this page shows.
    pub release: &'a Release,
    /// Newest released version.
    pub latest: &'a Release,
    /// Relative path from this page to the site root, e.g. `../../`.
    pub root: &'a str,
    /// The URL directory pages of this version live in: the tag, or `latest`.
    pub dir: &'a str,
    /// Slugs present in each version (by tag), for the version picker.
    pub pages_by_tag: &'a HashMap<String, Vec<NavEntry>>,
    pub canonical: Option<String>,
}

/// Escapes a page title and renders its `code` spans.
pub fn inline_title(s: &str) -> String {
    let mut out = String::new();
    for (i, part) in s.split('`').enumerate() {
        if i % 2 == 1 {
            out.push_str(&format!("<code>{}</code>", escape(part)));
        } else {
            out.push_str(&escape(part));
        }
    }
    out
}

fn head(ctx: &PageCtx, title: &str, extra: &str) -> String {
    let root = ctx.root;
    let canonical = ctx
        .canonical
        .as_ref()
        .map(|c| format!("<link rel=\"canonical\" href=\"{}\">\n", escape(c)))
        .unwrap_or_default();
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
{canonical}<meta name="docs-root" content="{root}">
<link rel="icon" href="{root}assets/favicon.svg" type="image/svg+xml">
<link rel="stylesheet" href="{root}assets/style.css">
<script>try{{var t=localStorage.getItem("theme");if(t)document.documentElement.dataset.theme=t}}catch(e){{}}</script>
{extra}</head>
"#,
        title = escape(&title.replace('`', "")),
    )
}

/// Release-line label for the picker, e.g. `3.0 (unreleased)`.
fn line_label(r: &Release) -> String {
    if r.released {
        r.version.line()
    } else {
        format!("{} (unreleased)", r.version.line())
    }
}

fn version_label(r: &Release, latest: &Release) -> String {
    if !r.released {
        format!("{} (unreleased)", r.tag)
    } else if r.tag == latest.tag {
        format!("{} (latest)", r.tag)
    } else {
        r.tag.clone()
    }
}

/// The version picker: a `<details>` list of links, grouped by release line, newest first.
/// `slug` is the current page; versions without it link to their home page.
fn version_picker(ctx: &PageCtx, slug: Option<&str>) -> String {
    let mut groups: Vec<(String, Vec<&Release>)> = Vec::new();
    for r in ctx.releases.iter().rev() {
        let label = line_label(r);
        match groups.last_mut() {
            Some((l, rs)) if *l == label => rs.push(r),
            _ => groups.push((label, vec![r])),
        }
    }
    let mut html = format!(
        "<details class=\"picker\" id=\"version-picker\"><summary aria-label=\"Choose a version\">\
         <span class=\"picker-current\">{}</span></summary><div class=\"picker-menu\">",
        escape(&version_label(ctx.release, ctx.latest))
    );
    for (label, rs) in groups {
        html.push_str(&format!(
            "<div class=\"picker-group\"><div class=\"picker-line\">{}</div><ul>",
            escape(&label)
        ));
        for r in rs {
            let has_page = slug.is_some_and(|s| {
                ctx.pages_by_tag
                    .get(&r.tag)
                    .is_some_and(|p| p.iter().any(|e| e.slug == s))
            });
            let href = match (slug, has_page) {
                (Some(s), true) => format!("{}{}/{s}/", ctx.root, r.tag),
                _ => format!("{}{}/", ctx.root, r.tag),
            };
            let current = if r.tag == ctx.release.tag {
                " aria-current=\"true\""
            } else {
                ""
            };
            let missing = if slug.is_some() && !has_page {
                " <span class=\"picker-note\">no such page</span>"
            } else {
                ""
            };
            html.push_str(&format!(
                "<li><a href=\"{href}\" data-version=\"{tag}\"{current}>{label}</a>{missing}</li>",
                tag = escape(&r.tag),
                label = escape(&version_label(r, ctx.latest)),
            ));
        }
        html.push_str("</ul></div>");
    }
    html.push_str("</div></details>");
    html
}

fn topbar(ctx: &PageCtx, slug: Option<&str>) -> String {
    let root = ctx.root;
    let tag = &ctx.release.tag;
    let diff_from = ctx
        .releases
        .iter()
        .rev()
        .find(|r| r.version < ctx.release.version)
        .unwrap_or(ctx.release);
    format!(
        r#"<header class="topbar">
<button class="menu-toggle" type="button" aria-label="Show pages" aria-controls="sidebar" aria-expanded="false"><svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M3 6h18M3 12h18M3 18h18" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round"/></svg></button>
<a class="brand" href="{root}{dir}/"><img src="{root}assets/favicon.svg" alt="" width="22" height="22"><span>{title}</span></a>
{picker}
{search}
<nav class="toplinks" aria-label="Tools">
<a href="{root}diff/?from={from}&amp;to={tag}">What changed</a>
<a href="{root}console/?version={tag}">Query console</a>
<a href="{repo}">GitHub</a>
<button class="theme-toggle" type="button" aria-label="Toggle dark mode"><svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" fill="currentColor"/></svg></button>
</nav>
</header>
"#,
        title = escape(&ctx.site.title),
        dir = escape(ctx.dir),
        picker = version_picker(ctx, slug),
        search = search_box(root, tag),
        from = escape(&diff_from.tag),
        repo = escape(&ctx.site.repo_url),
    )
}

/// The search field: a WAI-ARIA combobox over a listbox of results. Hidden until
/// `assets/search.js` runs, since search needs JavaScript.
fn search_box(root: &str, tag: &str) -> String {
    format!(
        r#"<div class="search" hidden>
<button class="search-toggle" type="button" aria-label="Search" aria-expanded="false" aria-controls="search-input"><svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="2" fill="none"/><path d="M20 20l-4-4" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg></button>
<input id="search-input" class="search-input" type="search" placeholder="Search" autocomplete="off" spellcheck="false" role="combobox" aria-expanded="false" aria-controls="search-results" aria-autocomplete="list" aria-label="Search the docs for {tag}">
<ul id="search-results" class="search-results" role="listbox" aria-label="Search results" hidden></ul>
<div id="search-status" class="sr-only" role="status"></div>
</div>
<script type="module" src="{root}assets/search.js"></script>"#,
        tag = escape(tag),
    )
}

fn sidebar(ctx: &PageCtx, nav: &[NavEntry], current: Option<&str>) -> String {
    let mut html = String::from("<nav class=\"sidebar\" id=\"sidebar\" aria-label=\"Pages\">");
    let mut sections: Vec<&str> = ctx.site.sections.iter().map(String::as_str).collect();
    for e in nav {
        if !sections.contains(&e.section.as_str()) {
            sections.push(&e.section);
        }
    }
    for section in sections {
        let entries: Vec<&NavEntry> = nav.iter().filter(|e| e.section == section).collect();
        if entries.is_empty() {
            continue;
        }
        html.push_str(&format!(
            "<div class=\"nav-section\"><h2>{}</h2><ul>",
            escape(section)
        ));
        for e in entries {
            let cur = if Some(e.slug.as_str()) == current {
                " aria-current=\"page\""
            } else {
                ""
            };
            html.push_str(&format!(
                "<li><a href=\"{}{}/{}/\" data-slug=\"{}\"{cur}>{}</a></li>",
                ctx.root,
                escape(ctx.dir),
                escape(&e.slug),
                escape(&e.slug),
                inline_title(&e.nav)
            ));
        }
        html.push_str("</ul></div>");
    }
    html.push_str("</nav>");
    html
}

fn banner(ctx: &PageCtx, slug: Option<&str>) -> String {
    let latest_href = |s: Option<&str>| match s {
        Some(s)
            if ctx
                .pages_by_tag
                .get(&ctx.latest.tag)
                .is_some_and(|p| p.iter().any(|e| e.slug == s)) =>
        {
            format!("{}{}/{s}/", ctx.root, ctx.latest.tag)
        }
        _ => format!("{}{}/", ctx.root, ctx.latest.tag),
    };
    if !ctx.release.released {
        format!(
            "<div class=\"banner banner-dev\" id=\"banner\">You are reading the docs for <strong>{}</strong>, \
             which is not released yet. The latest release is <a href=\"{}\">{}</a>.</div>",
            escape(&ctx.release.tag),
            latest_href(slug),
            escape(&ctx.latest.tag)
        )
    } else if ctx.release.tag != ctx.latest.tag {
        format!(
            "<div class=\"banner banner-old\" id=\"banner\">You are reading the docs for <strong>{}</strong>. \
             The latest release is <a href=\"{}\">{}</a>.</div>",
            escape(&ctx.release.tag),
            latest_href(slug),
            escape(&ctx.latest.tag)
        )
    } else {
        "<div id=\"banner\" hidden></div>".to_string()
    }
}

fn body_open(ctx: &PageCtx, kind: &str, slug: &str) -> String {
    format!(
        "<body data-kind=\"{kind}\" data-version=\"{}\" data-page=\"{}\" data-latest=\"{}\">\n\
         <a class=\"skip\" href=\"#content\">Skip to content</a>\n",
        escape(&ctx.release.tag),
        escape(slug),
        escape(&ctx.latest.tag)
    )
}

pub fn doc_page(
    ctx: &PageCtx,
    nav: &[NavEntry],
    entry: &NavEntry,
    frags: &[FragAt],
    blobs: &HashMap<String, String>,
) -> String {
    let title = format!("{} · {} {}", entry.title, ctx.site.title, ctx.release.tag);
    let mut html = head(
        ctx,
        &title,
        &format!(
            "<script type=\"module\" src=\"{}assets/app.js\"></script>\n",
            ctx.root
        ),
    );
    html.push_str(&body_open(ctx, "doc", &entry.slug));
    html.push_str(&topbar(ctx, Some(&entry.slug)));
    html.push_str("<div class=\"layout\">\n");
    html.push_str(&sidebar(ctx, nav, Some(&entry.slug)));
    html.push_str("<main id=\"content\" tabindex=\"-1\">\n");
    html.push_str(&banner(ctx, Some(&entry.slug)));
    let entries: Vec<TocEntry> = frags
        .iter()
        .flat_map(|f| toc_entries(&blobs[&f.blob]))
        .collect();
    let list = toc_list(&entries);
    html.push_str(&format!(
        "<article class=\"doc\"><h1 id=\"page-title\">{}</h1>\n{}<div id=\"frags\">\n",
        inline_title(&entry.title),
        toc_box(&list),
    ));
    for f in frags {
        html.push_str(&frag_html(f, &blobs[&f.blob]));
    }
    html.push_str("</div></article>\n");
    html.push_str(&format!(
        "<details class=\"assembled\"><summary>How this page was assembled</summary>\
         <p>This page is <span id=\"frag-count\">{n}</span> fragments. Minigraf picked them from \
         <a href=\"{root}docs.graph\">docs.graph</a> with this query, where the version is a point \
         on the valid-time axis:</p><pre><code id=\"assembled-query\">{q}</code></pre>\
         <p><a href=\"{root}console/?version={tag}&amp;page={slug}\">Run it in the query console</a></p></details>\n",
        n = frags.len(),
        root = ctx.root,
        q = escape(&frags_query(&entry.slug, ctx.release.version)),
        tag = escape(&ctx.release.tag),
        slug = escape(&entry.slug),
    ));
    html.push_str("</main>\n");
    html.push_str(&toc_nav(&list));
    html.push_str("</div>\n</body>\n</html>\n");
    html
}

/// A `##` or `###` heading, listed in the page navigator.
#[derive(Debug, PartialEq)]
pub struct TocEntry {
    pub level: u8,
    pub id: String,
    /// The heading's HTML without its anchor link: text and `code` spans only.
    pub html: String,
}

/// The `<h2 id>` and `<h3 id>` headings of rendered fragment HTML, in order.
///
/// `assets/toc.js` (`tocEntries`, `tocList`) builds the same list from the page after a version
/// switch. Keep the two in step: the end-to-end test "the browser's page navigator matches the
/// builder's on every page" compares them on every page of every version.
pub fn toc_entries(html: &str) -> Vec<TocEntry> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(p) = rest.find("<h") {
        rest = &rest[p + 2..];
        let level = match rest.as_bytes().first() {
            Some(b'2') => 2,
            Some(b'3') => 3,
            _ => continue,
        };
        let Some(after) = rest[1..].strip_prefix(" id=\"") else {
            continue;
        };
        let Some(q) = after.find('"') else { break };
        let id = &after[..q];
        let Some(gt) = after[q..].find('>') else {
            break;
        };
        let inner = &after[q + gt + 1..];
        let Some(end) = inner.find(&format!("</h{level}>")) else {
            break;
        };
        let text = &inner[..end];
        let text = text
            .find("<a class=\"anchor\"")
            .map_or(text, |a| &text[..a]);
        out.push(TocEntry {
            level,
            id: id.to_string(),
            html: keep_code(text).trim().to_string(),
        });
        rest = &inner[end..];
    }
    out
}

/// Drops every tag except `<code>` and `</code>`. Text is already escaped.
fn keep_code(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let Some(gt) = rest[lt..].find('>') else {
            return out;
        };
        let tag = &rest[lt..lt + gt + 1];
        if tag == "<code>" || tag == "</code>" {
            out.push_str(tag);
        }
        rest = &rest[lt + gt + 1..];
    }
    out.push_str(rest);
    out
}

/// The page navigator's list: `##` headings, each with its `###` headings in a nested list. The
/// stylesheet shows a nested list only for the section being read, which `assets/app.js` marks.
/// Empty when the page has fewer than two headings.
pub fn toc_list(entries: &[TocEntry]) -> String {
    if entries.len() < 2 {
        return String::new();
    }
    let mut html = String::new();
    let (mut item, mut sub) = (false, false);
    for e in entries {
        let link = format!("<a href=\"#{}\">{}</a>", e.id, e.html);
        if e.level == 3 && item {
            if !sub {
                html.push_str("<ul>");
                sub = true;
            }
            html.push_str(&format!("<li>{link}</li>"));
            continue;
        }
        if sub {
            html.push_str("</ul>");
            sub = false;
        }
        if item {
            html.push_str("</li>");
        }
        html.push_str(&format!("<li>{link}"));
        item = true;
    }
    if sub {
        html.push_str("</ul>");
    }
    if item {
        html.push_str("</li>");
    }
    html
}

fn hidden_if_empty(list: &str) -> &'static str {
    if list.is_empty() { " hidden" } else { "" }
}

/// The page navigator as a column beside the page, on wide screens.
pub fn toc_nav(list: &str) -> String {
    format!(
        "<nav class=\"toc\" id=\"toc\" aria-labelledby=\"toc-title\"{}>\
         <div class=\"toc-title\" id=\"toc-title\">On this page</div>\
         <ul class=\"toc-list\">{list}</ul></nav>\n",
        hidden_if_empty(list)
    )
}

/// The page navigator as a collapsed box under the page title, on screens too narrow for the
/// column.
pub fn toc_box(list: &str) -> String {
    format!(
        "<details class=\"toc-box\" id=\"toc-box\"{}><summary>On this page</summary>\
         <nav aria-label=\"On this page\"><ul class=\"toc-list\">{list}</ul></nav></details>\n",
        hidden_if_empty(list)
    )
}

pub fn frag_html(f: &FragAt, body: &str) -> String {
    let badge = f
        .added_in
        .as_ref()
        .map(|v| format!("<span class=\"badge-added\">Added in {}</span>", escape(v)))
        .unwrap_or_default();
    format!(
        "<section class=\"frag\" data-blob=\"{}\" data-order=\"{}\">{badge}{body}</section>\n",
        escape(&f.blob),
        f.order
    )
}

/// A page under `<version>/` or the site root that forwards elsewhere, keeping the fragment.
pub fn redirect(to: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"refresh\" content=\"0; url={to}\">\
         <link rel=\"canonical\" href=\"{to}\"><title>Redirecting</title>\
         <script>location.replace(\"{to}\"+location.hash)</script></head>\
         <body><a href=\"{to}\">Continue</a></body></html>\n"
    )
}

fn version_options(ctx: &PageCtx, selected: &str) -> String {
    let mut html = String::new();
    for r in ctx.releases.iter().rev() {
        let sel = if r.tag == selected { " selected" } else { "" };
        html.push_str(&format!(
            "<option value=\"{tag}\"{sel}>{label}</option>",
            tag = escape(&r.tag),
            label = escape(&version_label(r, ctx.latest))
        ));
    }
    html
}

pub fn diff_page(ctx: &PageCtx, nav: &[NavEntry]) -> String {
    let prev = ctx
        .releases
        .iter()
        .rev()
        .find(|r| r.version < ctx.release.version)
        .unwrap_or(ctx.release);
    let mut html = head(
        ctx,
        &format!("What changed · {}", ctx.site.title),
        &format!(
            "<script type=\"module\" src=\"{}assets/diff.js\"></script>\n",
            ctx.root
        ),
    );
    html.push_str(&body_open(ctx, "diff", ""));
    html.push_str(&topbar(ctx, None));
    html.push_str("<div class=\"layout\">\n");
    html.push_str(&sidebar(ctx, nav, None));
    html.push_str(&format!(
        r#"<main id="content" tabindex="-1">
<article class="doc tool">
<h1>What changed</h1>
<p class="lede">Pick two versions. Every fragment of the docs carries a valid-time window, so the
changes between two versions are the fragments whose window starts or ends between them.</p>
<form class="diff-form" id="diff-form">
<label>From <select name="from" id="diff-from">{from}</select></label>
<span class="arrow" aria-hidden="true">→</span>
<label>To <select name="to" id="diff-to">{to}</select></label>
<button type="submit">Compare</button>
</form>
<noscript><p class="note">The comparison runs Minigraf in your browser and needs JavaScript.</p></noscript>
<div id="diff-status" class="status" role="status"></div>
<div id="diff-summary"></div>
<div id="diff-results"></div>
<details class="assembled" id="diff-queries" hidden><summary>Queries used</summary><pre><code id="diff-query"></code></pre></details>
</article>
</main>
</div>
</body>
</html>
"#,
        from = version_options(ctx, &prev.tag),
        to = version_options(ctx, &ctx.release.tag),
    ));
    html
}

pub fn console_page(ctx: &PageCtx, nav: &[NavEntry]) -> String {
    let mut html = head(
        ctx,
        &format!("Query console · {}", ctx.site.title),
        &format!(
            "<script type=\"module\" src=\"{}assets/console.js\"></script>\n",
            ctx.root
        ),
    );
    html.push_str(&body_open(ctx, "console", ""));
    html.push_str(&topbar(ctx, None));
    html.push_str("<div class=\"layout\">\n");
    html.push_str(&sidebar(ctx, nav, None));
    html.push_str(&format!(
        r#"<main id="content" tabindex="-1">
<article class="doc tool">
<h1>Query console</h1>
<p class="lede">These docs are indexed by a Minigraf database,
<a href="{root}docs.graph">docs.graph</a>. It is loaded into memory in your browser, so you can
query it here. Changes you make are lost when you leave the page.</p>
<details class="schema"><summary>Schema</summary>
<table>
<thead><tr><th>Entity</th><th>Attributes</th><th>Valid time</th></tr></thead>
<tbody>
<tr><td>version</td><td><code>:version/tag</code> <code>:version/at</code> <code>:version/line</code> <code>:version/date</code> <code>:version/released</code></td><td>always (use <code>:any-valid-time</code>)</td></tr>
<tr><td>page</td><td><code>:page/slug</code> <code>:page/title</code> <code>:page/section</code> <code>:page/order</code></td><td>the versions the page exists in</td></tr>
<tr><td>fragment</td><td><code>:frag/page</code> <code>:frag/order</code> <code>:frag/blob</code> <code>:frag/heading</code> <code>:frag/added-in</code></td><td>the versions the fragment is part of its page</td></tr>
</tbody>
</table>
<p>A version is a point on the valid-time axis: <code>vMAJOR.MINOR.PATCH</code> is
<code>(2000+MAJOR)-01-01</code> plus MINOR days plus PATCH seconds. The version picker below fills in
<code>:valid-at</code> for you.</p>
</details>
<form id="console-form" class="console-form">
<div class="console-row">
<label>Version <select id="console-version">{versions}</select></label>
<label>Example <select id="console-example"></select></label>
</div>
<textarea id="console-input" spellcheck="false" autocapitalize="off" autocomplete="off" rows="7" aria-label="Datalog"></textarea>
<div class="console-row"><button type="submit">Run</button><span class="hint">Ctrl+Enter</span></div>
</form>
<noscript><p class="note">The console runs Minigraf in your browser and needs JavaScript.</p></noscript>
<div id="console-status" class="status" role="status"></div>
<div id="console-output"></div>
</article>
</main>
</div>
</body>
</html>
"#,
        root = ctx.root,
        versions = version_options(ctx, &ctx.release.tag),
    ));
    html
}

pub fn not_found(site: &Site) -> String {
    let base = site.base_path();
    let base_js = format!("{base:?}");
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Not found · {title}</title>
<link rel="stylesheet" href="{base}assets/style.css">
<script>
// Old wiki-style paths: /<Page-Name> or /<page-name>/ without a version go to latest/.
(function () {{
  var base = {base_js};
  var p = location.pathname;
  if (p.indexOf(base) !== 0) return;
  var rest = p.slice(base.length).replace(/\/+$/, "");
  if (rest && rest.indexOf("/") < 0 && !/^v\d/.test(rest) && rest !== "latest") {{
    location.replace(base + "latest/" + rest.toLowerCase() + "/" + location.hash);
  }}
}})();
</script>
</head>
<body class="plain">
<main class="notfound">
<h1>Page not found</h1>
<p>That page does not exist in this version of the docs.</p>
<p><a href="{base}latest/">Go to the latest docs</a></p>
</main>
</body>
</html>
"#,
        title = escape(&site.title),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toc_lists_h2_and_nested_h3() {
        let html = concat!(
            "<p>Intro</p><hr />",
            "<h2 id=\"setup\">Setup<a class=\"anchor\" href=\"#setup\" aria-label=\"x\">#</a></h2>\n",
            "<h3 id=\"as-of\"><code>:as-of</code> &amp; <a href=\"../x/\">more</a>",
            "<a class=\"anchor\" href=\"#as-of\" aria-label=\"x\">#</a></h3>\n",
            "<h4 id=\"deep\">Deep</h4><h2 id=\"next\">Next</h2>",
        );
        let entries = toc_entries(html);
        assert_eq!(
            entries,
            vec![
                TocEntry {
                    level: 2,
                    id: "setup".into(),
                    html: "Setup".into()
                },
                TocEntry {
                    level: 3,
                    id: "as-of".into(),
                    html: "<code>:as-of</code> &amp; more".into()
                },
                TocEntry {
                    level: 2,
                    id: "next".into(),
                    html: "Next".into()
                },
            ]
        );
        assert_eq!(
            toc_list(&entries),
            "<li><a href=\"#setup\">Setup</a><ul><li><a href=\"#as-of\">\
             <code>:as-of</code> &amp; more</a></li></ul></li>\
             <li><a href=\"#next\">Next</a></li>"
        );
    }

    #[test]
    fn toc_hidden_with_one_heading() {
        let list = toc_list(&toc_entries("<h2 id=\"only\">Only</h2>"));
        assert_eq!(list, "");
        assert!(toc_nav(&list).contains(" hidden>"));
        assert!(toc_box(&list).contains(" hidden>"));
        // A `###` before any `##` is listed at the top level.
        let list = toc_list(&toc_entries("<h3 id=\"a\">A</h3><h2 id=\"b\">B</h2>"));
        assert!(list.starts_with("<li><a href=\"#a\">A</a></li><li>"));
        assert!(!toc_nav(&list).contains("hidden"));
    }

    #[test]
    fn search_box_is_an_aria_combobox() {
        let html = search_box("../../", "v3.0.0");
        for attr in [
            r#"<div class="search" hidden>"#,
            r#"role="combobox""#,
            r#"aria-expanded="false" aria-controls="search-results""#,
            r#"aria-autocomplete="list""#,
            r#"aria-label="Search the docs for v3.0.0""#,
            r#"<ul id="search-results" class="search-results" role="listbox""#,
            r#"role="status""#,
            r#"<script type="module" src="../../assets/search.js"></script>"#,
        ] {
            assert!(html.contains(attr), "missing {attr}");
        }
    }
}
