//! `docs.graph`: the version index, written and read with Datalog.

use crate::config::Release;
use crate::version::{Version, Window};
use anyhow::{Context, Result, bail};
use minigraf::{Minigraf, QueryResult, Value};
use std::path::Path;

/// Valid-time floor for windows with no start: v0.0.0. Without an explicit `:valid-from`,
/// Minigraf would start the window at the transaction time, after every version.
const FLOOR: Version = Version {
    major: 0,
    minor: 0,
    patch: 0,
};

#[derive(Debug, Clone)]
pub struct PageFacts {
    pub slug: String,
    pub title: String,
    pub nav: String,
    pub section: String,
    pub order: i64,
    pub window: Window,
    pub frags: Vec<FragFacts>,
}

#[derive(Debug, Clone)]
pub struct FragFacts {
    pub order: i64,
    pub blob: String,
    pub heading: Option<String>,
    pub window: Window,
    pub added_in: Option<Version>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavEntry {
    pub slug: String,
    pub title: String,
    pub nav: String,
    pub section: String,
    pub order: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FragAt {
    pub order: i64,
    pub blob: String,
    pub added_in: Option<String>,
}

pub fn edn_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn window_map(w: Window) -> String {
    let from = w.from.unwrap_or(FLOOR).time();
    match w.to {
        Some(to) => format!("{{:valid-from \"{from}\" :valid-to \"{}\"}}", to.time()),
        None => format!("{{:valid-from \"{from}\"}}"),
    }
}

fn keyword_part(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

pub struct DocsGraph {
    db: Minigraf,
}

impl DocsGraph {
    pub fn create(path: &Path) -> Result<Self> {
        for p in [path.to_path_buf(), path.with_extension("graph.wal")] {
            if p.exists() {
                std::fs::remove_file(&p).with_context(|| format!("removing {}", p.display()))?;
            }
        }
        let db = Minigraf::open(path).context("opening docs.graph")?;
        Ok(DocsGraph { db })
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        Ok(DocsGraph {
            db: Minigraf::in_memory()?,
        })
    }

    fn exec(&self, datalog: &str) -> Result<QueryResult> {
        self.db
            .execute(datalog)
            .with_context(|| format!("executing: {}", truncate(datalog)))
    }

    pub fn write_versions(&self, releases: &[Release]) -> Result<()> {
        let mut facts = String::new();
        for r in releases {
            let e = format!(":docs/version-{}", keyword_part(&r.tag));
            let at = r.version.time();
            let all = window_map(Window::ALL);
            facts.push_str(&format!(
                "[{e} :version/tag {} {all}] [{e} :version/at {} {all}] \
                 [{e} :version/line {} {all}] [{e} :version/date {} {all}] \
                 [{e} :version/released {} {all}]\n",
                edn_str(&r.tag),
                edn_str(&at),
                edn_str(&r.version.line()),
                edn_str(&r.date),
                r.released
            ));
        }
        self.exec(&format!("(transact [{facts}])"))?;
        Ok(())
    }

    /// One transaction per page: its own facts and its fragments'.
    pub fn write_page(&self, page: &PageFacts, entity_index: usize) -> Result<()> {
        let slug = keyword_part(&page.slug);
        let pe = format!(":docs/page-{slug}-{entity_index}");
        let pw = window_map(page.window);
        let mut facts = format!(
            "[{pe} :page/slug {} {pw}] [{pe} :page/title {} {pw}] \
             [{pe} :page/nav {} {pw}] [{pe} :page/section {} {pw}] \
             [{pe} :page/order {} {pw}]\n",
            edn_str(&page.slug),
            edn_str(&page.title),
            edn_str(&page.nav),
            edn_str(&page.section),
            page.order
        );
        for (k, f) in page.frags.iter().enumerate() {
            let fe = format!(":docs/frag-{slug}-{entity_index}-{k}");
            let fw = window_map(f.window);
            facts.push_str(&format!(
                "[{fe} :frag/page {} {fw}] [{fe} :frag/order {} {fw}] \
                 [{fe} :frag/blob {} {fw}] [{fe} :frag/heading {} {fw}] \
                 [{fe} :frag/added-in {} {fw}]\n",
                edn_str(&page.slug),
                f.order,
                edn_str(&f.blob),
                edn_str(f.heading.as_deref().unwrap_or("")),
                edn_str(&f.added_in.map(|v| v.to_string()).unwrap_or_default()),
            ));
        }
        self.exec(&format!("(transact [{facts}])"))?;
        Ok(())
    }

    pub fn checkpoint(&self) -> Result<()> {
        self.db.checkpoint().context("checkpointing docs.graph")
    }

    fn rows(&self, datalog: &str) -> Result<Vec<Vec<Value>>> {
        match self.exec(datalog)? {
            QueryResult::QueryResults { results, .. } => Ok(results),
            _ => bail!("expected query results"),
        }
    }

    /// The sidebar at a version, in display order within each section.
    pub fn nav_at(&self, v: Version) -> Result<Vec<NavEntry>> {
        let rows = self.rows(&nav_query(v))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(NavEntry {
                slug: string(&row[0])?,
                title: string(&row[1])?,
                nav: string(&row[2])?,
                section: string(&row[3])?,
                order: int(&row[4])?,
            });
        }
        out.sort_by(|a, b| (a.order, &a.slug).cmp(&(b.order, &b.slug)));
        Ok(out)
    }

    /// A page's fragments at a version, in order.
    pub fn frags_at(&self, slug: &str, v: Version) -> Result<Vec<FragAt>> {
        let rows = self.rows(&frags_query(slug, v))?;
        let mut out = Vec::new();
        for row in rows {
            let added = string(&row[2])?;
            out.push(FragAt {
                order: int(&row[0])?,
                blob: string(&row[1])?,
                added_in: (!added.is_empty()).then_some(added),
            });
        }
        out.sort_by_key(|f| f.order);
        for w in out.windows(2) {
            if w[0].order == w[1].order {
                bail!(
                    "page {slug} has two fragments at position {} in {v}",
                    w[0].order
                );
            }
        }
        Ok(out)
    }
}

/// The queries are shared with the browser (see `assets/app.js`), which builds the same text.
pub fn nav_query(v: Version) -> String {
    format!(
        "(query [:find ?slug ?title ?nav ?section ?order :valid-at \"{}\" \
         :where [?p :page/slug ?slug] [?p :page/title ?title] [?p :page/nav ?nav] \
         [?p :page/section ?section] [?p :page/order ?order]])",
        v.time()
    )
}

pub fn frags_query(slug: &str, v: Version) -> String {
    format!(
        "(query [:find ?order ?blob ?added :valid-at \"{}\" \
         :where [?f :frag/page {}] [?f :frag/order ?order] [?f :frag/blob ?blob] \
         [?f :frag/added-in ?added]])",
        v.time(),
        edn_str(slug)
    )
}

fn string(v: &Value) -> Result<String> {
    match v {
        Value::String(s) => Ok(s.clone()),
        _ => bail!("expected a string in query results"),
    }
}

fn int(v: &Value) -> Result<i64> {
    match v {
        Value::Integer(i) => Ok(*i),
        _ => bail!("expected an integer in query results"),
    }
}

fn truncate(s: &str) -> String {
    if s.len() > 300 {
        format!("{}…", &s[..s.floor_char_boundary(300)])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn frag(order: i64, blob: &str, window: Window) -> FragFacts {
        FragFacts {
            order,
            blob: blob.into(),
            heading: None,
            window,
            added_in: None,
        }
    }

    #[test]
    fn valid_at_selects_fragments_by_version() {
        let g = DocsGraph::in_memory().unwrap();
        let old = Window {
            from: None,
            to: Some(v("v3.0.0")),
        };
        let new = Window {
            from: Some(v("v3.0.0")),
            to: None,
        };
        let backport = Window {
            from: Some(v("v2.0.3")),
            to: None,
        };
        g.write_page(
            &PageFacts {
                slug: "arch".into(),
                title: "Arch \"quoted\"".into(),
                nav: "Arch".into(),
                section: "Reference".into(),
                order: 1,
                window: Window::ALL,
                frags: vec![
                    frag(0, "common", Window::ALL),
                    frag(1, "v7", old),
                    frag(2, "v8", new),
                    frag(3, "fix", backport),
                ],
            },
            0,
        )
        .unwrap();
        g.write_page(
            &PageFacts {
                slug: "v3-only".into(),
                title: "New".into(),
                nav: "New".into(),
                section: "Reference".into(),
                order: 2,
                window: new,
                frags: vec![frag(0, "n", Window::ALL)],
            },
            0,
        )
        .unwrap();
        let blobs = |ver: &str| -> Vec<String> {
            g.frags_at("arch", v(ver))
                .unwrap()
                .into_iter()
                .map(|f| f.blob)
                .collect()
        };
        assert_eq!(blobs("v2.0.0"), vec!["common", "v7"]);
        assert_eq!(blobs("v2.0.3"), vec!["common", "v7", "fix"]);
        assert_eq!(blobs("v2.0.9"), vec!["common", "v7", "fix"]);
        assert_eq!(blobs("v3.0.0"), vec!["common", "v8", "fix"]);
        assert_eq!(g.nav_at(v("v2.0.4")).unwrap().len(), 1);
        let nav3 = g.nav_at(v("v3.0.0")).unwrap();
        assert_eq!(nav3.len(), 2);
        assert_eq!(nav3[0].title, "Arch \"quoted\"");
    }
}
