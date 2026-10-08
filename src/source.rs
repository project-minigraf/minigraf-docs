//! Authored pages: front matter, version annotations and fragment splitting.

use crate::version::{Version, Window};
use anyhow::{Context, Result, bail};

/// A page as written in `content/`.
#[derive(Debug)]
pub struct AuthoredPage {
    pub slug: String,
    pub title: String,
    /// Sidebar label; defaults to the title.
    pub nav: String,
    pub section: String,
    pub order: i64,
    pub window: Window,
    pub frags: Vec<RawFrag>,
}

/// A run of Markdown with one validity window.
#[derive(Debug, Clone, PartialEq)]
pub struct RawFrag {
    pub markdown: String,
    pub window: Window,
    /// Set when an annotation inside the page made this fragment start later than the page:
    /// rendered as an "Added in" badge.
    pub added_in: Option<Version>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Annotation {
    Open(Window, Option<Version>),
    End,
}

fn parse_annotation(line: &str) -> Result<Option<Annotation>> {
    let t = line.trim();
    let Some(inner) = t
        .strip_prefix("<!--")
        .and_then(|s| s.strip_suffix("-->"))
        .map(str::trim)
    else {
        return Ok(None);
    };
    let Some(directive) = inner.strip_prefix('@') else {
        return Ok(None);
    };
    let (name, arg) = directive
        .split_once(char::is_whitespace)
        .map(|(n, a)| (n, a.trim()))
        .unwrap_or((directive, ""));
    let ann = match name {
        "end" => Annotation::End,
        "since" => {
            let v = Version::parse(arg)?;
            Annotation::Open(
                Window {
                    from: Some(v),
                    to: None,
                },
                Some(v),
            )
        }
        "until" => Annotation::Open(
            Window {
                from: None,
                to: Some(Version::parse(arg)?),
            },
            None,
        ),
        "range" => {
            let (a, b) = arg
                .split_once("..")
                .with_context(|| format!("`@range {arg}` must be vA..vB"))?;
            let from = Version::parse(a.trim())?;
            let to = Version::parse(b.trim())?;
            Annotation::Open(
                Window {
                    from: Some(from),
                    to: Some(to),
                },
                Some(from),
            )
        }
        other => bail!("unknown annotation `@{other}`"),
    };
    Ok(Some(ann))
}

/// Tracks fenced code blocks so headings and annotations inside them are ignored.
#[derive(Default)]
pub struct FenceTracker {
    open: Option<(char, usize)>,
}

impl FenceTracker {
    /// Feeds one line; returns true if the line is inside a fence (including its delimiters).
    pub fn feed(&mut self, line: &str) -> bool {
        let t = line.trim_start();
        let indent = line.len() - t.len();
        let fence = |c: char| {
            let n = t.chars().take_while(|&x| x == c).count();
            (indent < 4 && n >= 3).then_some(n)
        };
        match self.open {
            Some((c, n)) => {
                if let Some(m) = fence(c)
                    && m >= n
                    && t[m..].trim().is_empty()
                {
                    self.open = None;
                }
                true
            }
            None => {
                for c in ['`', '~'] {
                    if let Some(n) = fence(c) {
                        self.open = Some((c, n));
                        return true;
                    }
                }
                false
            }
        }
    }
}

/// ATX heading level of a line outside a fence, if it is one.
pub fn heading_level(line: &str) -> Option<u8> {
    let t = line.trim_start();
    if line.len() - t.len() >= 4 {
        return None;
    }
    let n = t.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&n) && (t.len() == n || t[n..].starts_with(' ')) {
        Some(n as u8)
    } else {
        None
    }
}

fn parse_front_matter(text: &str) -> Result<(Vec<(String, String)>, &str)> {
    let Some(rest) = text.strip_prefix("---\n") else {
        bail!("page must start with a `---` front-matter block");
    };
    let end = rest
        .find("\n---\n")
        .context("front-matter block is not closed with `---`")?;
    let mut fields = Vec::new();
    for line in rest[..end].lines() {
        let line = line.split(" #").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let (k, v) = line
            .split_once(':')
            .with_context(|| format!("front-matter line `{line}` is not `key: value`"))?;
        fields.push((k.trim().to_string(), v.trim().to_string()));
    }
    Ok((fields, &rest[end + 5..]))
}

pub fn parse_page(slug: &str, text: &str) -> Result<AuthoredPage> {
    let (fields, body) = parse_front_matter(text)?;
    let get = |k: &str| fields.iter().find(|(f, _)| f == k).map(|(_, v)| v.as_str());
    for (k, _) in &fields {
        if !["title", "nav", "section", "order", "since", "until"].contains(&k.as_str()) {
            bail!("unknown front-matter key `{k}`");
        }
    }
    let window = Window {
        from: get("since").map(Version::parse).transpose()?,
        to: get("until").map(Version::parse).transpose()?,
    };
    if window.is_empty() {
        bail!("page window is empty");
    }
    Ok(AuthoredPage {
        slug: slug.to_string(),
        title: get("title")
            .context("front matter needs `title`")?
            .to_string(),
        nav: get("nav").or(get("title")).unwrap_or_default().to_string(),
        section: get("section")
            .context("front matter needs `section`")?
            .to_string(),
        order: get("order")
            .context("front matter needs `order`")?
            .parse()
            .context("`order` must be an integer")?,
        window,
        frags: split_annotated(body, window, 3)?,
    })
}

/// Splits Markdown into fragments at headings of level `split` or above and at annotation
/// boundaries. `page` is the page's own window; annotations narrow it.
pub fn split_annotated(body: &str, page: Window, split: u8) -> Result<Vec<RawFrag>> {
    split_impl(body, page, split, true)
}

/// Splits Markdown at headings only; HTML comments are plain content.
pub fn split_plain(body: &str, split: u8) -> Result<Vec<String>> {
    Ok(split_impl(body, Window::ALL, split, false)?
        .into_iter()
        .map(|f| f.markdown)
        .collect())
}

fn split_impl(body: &str, page: Window, split: u8, annotations: bool) -> Result<Vec<RawFrag>> {
    let mut frags = Vec::new();
    let mut stack: Vec<(Window, Option<Version>)> = Vec::new();
    let mut fences = FenceTracker::default();
    let mut cur = String::new();

    let current = |stack: &[(Window, Option<Version>)]| -> (Window, Option<Version>) {
        let mut w = page;
        let mut added = None;
        for (sw, since) in stack {
            w = w.intersect(*sw);
            if let Some(s) = since
                && page.from.is_none_or(|pf| *s > pf)
            {
                added = Some(*s);
            }
        }
        (w, added)
    };
    let flush = |cur: &mut String,
                 frags: &mut Vec<RawFrag>,
                 stack: &[(Window, Option<Version>)]|
     -> Result<()> {
        if !cur.trim().is_empty() {
            let (window, added_in) = current(stack);
            if window.is_empty() {
                bail!("annotated block is outside the page's versions");
            }
            frags.push(RawFrag {
                markdown: std::mem::take(cur).trim_matches('\n').to_string() + "\n",
                window,
                added_in,
            });
        }
        cur.clear();
        Ok(())
    };

    for (n, line) in body.lines().enumerate() {
        let in_fence = fences.feed(line);
        if !in_fence {
            let ann = if annotations {
                parse_annotation(line).with_context(|| format!("line {}", n + 1))?
            } else {
                None
            };
            match ann {
                Some(Annotation::Open(w, since)) => {
                    flush(&mut cur, &mut frags, &stack)?;
                    stack.push((w, since));
                    continue;
                }
                Some(Annotation::End) => {
                    flush(&mut cur, &mut frags, &stack)?;
                    if stack.pop().is_none() {
                        bail!("line {}: `@end` without an open annotation", n + 1);
                    }
                    continue;
                }
                None => {}
            }
            if heading_level(line).is_some_and(|l| l <= split) {
                flush(&mut cur, &mut frags, &stack)?;
            }
        }
        cur.push_str(line);
        cur.push('\n');
    }
    if fences.open.is_some() {
        bail!("unclosed code fence");
    }
    if !stack.is_empty() {
        bail!("annotation opened but never closed with `@end`");
    }
    flush(&mut cur, &mut frags, &stack)?;
    Ok(frags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn splits_at_headings_and_annotations() {
        let text = "---\ntitle: T\nsection: S\norder: 1\nsince: v2.0.0\n---\n\
            intro\n\n## A\n\nshared\n\n<!-- @until v3.0.0 -->\nold\n<!-- @end -->\n\
            <!-- @since v3.0.0 -->\nnew\n\n### Sub\n\nmore new\n<!-- @end -->\n## B\nb\n";
        let p = parse_page("t", text).unwrap();
        let mds: Vec<&str> = p.frags.iter().map(|f| f.markdown.as_str()).collect();
        assert_eq!(
            mds,
            vec![
                "intro\n",
                "## A\n\nshared\n",
                "old\n",
                "new\n",
                "### Sub\n\nmore new\n",
                "## B\nb\n"
            ]
        );
        assert_eq!(p.frags[2].window.to, Some(v("v3.0.0")));
        assert_eq!(p.frags[2].added_in, None);
        assert_eq!(p.frags[3].window.from, Some(v("v3.0.0")));
        assert_eq!(p.frags[3].added_in, Some(v("v3.0.0")));
        assert_eq!(p.frags[4].added_in, Some(v("v3.0.0")));
        assert_eq!(p.frags[5].window, p.window);
    }

    #[test]
    fn ignores_headings_and_annotations_in_fences() {
        let body = "## A\n```\n## not a heading\n<!-- @since v3.0.0 -->\n```\nafter\n";
        let f = split_annotated(body, Window::ALL, 3).unwrap();
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn rejects_bad_annotations() {
        assert!(split_annotated("<!-- @since v3.0.0 -->\nx\n", Window::ALL, 3).is_err());
        assert!(split_annotated("x\n<!-- @end -->\n", Window::ALL, 3).is_err());
        assert!(
            split_annotated("<!-- @sinc v3.0.0 -->\nx\n<!-- @end -->", Window::ALL, 3).is_err()
        );
        let page = Window {
            from: None,
            to: Some(v("v3.0.0")),
        };
        assert!(split_annotated("<!-- @since v3.0.0 -->\nx\n<!-- @end -->\n", page, 3).is_err());
    }

    #[test]
    fn ordinary_comments_are_content() {
        let f = split_annotated("<!-- note -->\ntext\n", Window::ALL, 3).unwrap();
        assert_eq!(f[0].markdown, "<!-- note -->\ntext\n");
    }
}
