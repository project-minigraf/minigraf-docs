//! `site.toml`: versions, snapshot pages and site settings.

use crate::version::Version;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub site: Site,
    #[serde(rename = "version")]
    pub versions: Vec<VersionEntry>,
    #[serde(default, rename = "snapshot")]
    pub snapshots: Vec<SnapshotEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    pub title: String,
    /// Absolute URL of the site root, ending in `/`. Used for the sitemap, canonical links and
    /// the 404 page; everything else uses relative URLs.
    pub url: String,
    pub repo_url: String,
    /// Sidebar sections, in display order.
    pub sections: Vec<String>,
    /// Slug of the page `<version>/` opens.
    pub home: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionEntry {
    pub tag: String,
    /// Release date (YYYY-MM-DD); empty for an unreleased version.
    #[serde(default)]
    pub date: String,
    #[serde(default = "yes")]
    pub released: bool,
    /// Git ref snapshot pages are read from; defaults to the tag.
    #[serde(default)]
    pub git_ref: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotEntry {
    pub slug: String,
    pub title: String,
    pub section: String,
    pub order: i64,
    /// Path of the Markdown file in the source repository.
    pub path: String,
    /// Split into fragments at headings of this level or above (2 = `##`).
    #[serde(default = "two")]
    pub split: u8,
    /// Give `###` and deeper headings ids prefixed with their `##` heading's id. Needed when
    /// the same subheading repeats under every `##` (a changelog's `### Fixed`).
    #[serde(default)]
    pub scoped_ids: bool,
    /// First version the file exists in.
    pub since: String,
}

fn two() -> u8 {
    2
}

#[derive(Debug, Clone)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    pub date: String,
    pub released: bool,
    pub git_ref: String,
}

impl Site {
    /// The path part of `url`, e.g. `/minigraf-docs/`.
    pub fn base_path(&self) -> &str {
        let after = &self.url[self.url.find("://").map_or(0, |i| i + 3)..];
        after.find('/').map_or("/", |i| &after[i..])
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        if !cfg.site.url.ends_with('/') || !cfg.site.url.contains("://") {
            bail!("site.url must be an absolute URL ending in `/`");
        }
        if cfg.versions.is_empty() {
            bail!("site.toml lists no versions");
        }
        Ok(cfg)
    }

    /// Versions in semver order.
    pub fn releases(&self) -> Result<Vec<Release>> {
        let mut out = Vec::new();
        for v in &self.versions {
            out.push(Release {
                version: Version::parse(&v.tag)?,
                tag: v.tag.clone(),
                date: v.date.clone(),
                released: v.released,
                git_ref: v.git_ref.clone().unwrap_or_else(|| v.tag.clone()),
            });
        }
        out.sort_by_key(|r| r.version);
        for w in out.windows(2) {
            if w[0].version == w[1].version {
                bail!("version {} is listed twice", w[0].tag);
            }
        }
        Ok(out)
    }
}
