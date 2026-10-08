//! Builds the Minigraf docs site.
//!
//! ```text
//! minigraf-docs build [--root DIR] [--out DIR] [--source-repo DIR]
//! ```
//!
//! Reads `site.toml` and `content/` under `--root` (default `.`), reads snapshot pages from the
//! Minigraf repository at `--source-repo` (default `../minigraf`), and writes the site to
//! `--out` (default `dist`). See DESIGN.md.

mod config;
mod graph;
mod render;
mod site;
mod snapshot;
mod source;
mod version;

use anyhow::{Context, Result, bail};
use config::{Config, Release};
use graph::{DocsGraph, FragFacts, NavEntry, PageFacts};
use render::{IdAllocator, Link, LinkMode, Rendered, render};
use sha2::{Digest, Sha256};
use site::PageCtx;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use version::Window;

struct Args {
    root: PathBuf,
    out: PathBuf,
    source_repo: PathBuf,
}

fn parse_args() -> Result<Args> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("build") => {}
        _ => bail!("usage: minigraf-docs build [--root DIR] [--out DIR] [--source-repo DIR]"),
    }
    let mut out = Args {
        root: PathBuf::from("."),
        out: PathBuf::from("dist"),
        source_repo: PathBuf::from("../minigraf"),
    };
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .with_context(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--root" => out.root = value.into(),
            "--out" => out.out = value.into(),
            "--source-repo" => out.source_repo = value.into(),
            other => bail!("unknown option {other}"),
        }
    }
    Ok(out)
}

fn main() {
    if let Err(e) = parse_args().and_then(|a| build(&a)) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn hash(html: &str) -> String {
    Sha256::digest(html.as_bytes())[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)
                .with_context(|| format!("copying {}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Everything rendered, before it goes into the graph.
struct Sources {
    pages: Vec<PageFacts>,
    blobs: HashMap<String, Rendered>,
    /// Pages whose broken links are warnings rather than errors (snapshots).
    lenient: HashSet<String>,
}

fn load_sources(cfg: &Config, releases: &[Release], args: &Args) -> Result<Sources> {
    let content = args.root.join("content");
    let mut authored = Vec::new();
    let mut files: Vec<PathBuf> = fs::read_dir(&content)
        .with_context(|| format!("reading {}", content.display()))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    files.sort();
    for path in files {
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let slug = path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("bad file name")?
            .to_string();
        let text = fs::read_to_string(&path)?;
        authored
            .push(source::parse_page(&slug, &text).with_context(|| format!("{}", path.display()))?);
    }

    let mut slugs = BTreeSet::new();
    for slug in authored
        .iter()
        .map(|p| &p.slug)
        .chain(cfg.snapshots.iter().map(|s| &s.slug))
    {
        if !slugs.insert(slug.clone()) {
            bail!("two pages have the slug `{slug}`");
        }
    }

    let mut blobs: HashMap<String, Rendered> = HashMap::new();
    let mut pages = Vec::new();
    for page in &authored {
        let mut ids = IdAllocator::default();
        let mut frags = Vec::new();
        for (order, f) in page.frags.iter().enumerate() {
            let r = render(
                &f.markdown,
                &slugs,
                &mut ids,
                f.window,
                false,
                &LinkMode::Authored,
            )
            .with_context(|| format!("content/{}.md, fragment {order}", page.slug))?;
            let h = hash(&r.html);
            frags.push(FragFacts {
                order: order as i64,
                blob: h.clone(),
                heading: r.heading.clone(),
                window: f.window,
                added_in: f.added_in,
            });
            blobs.entry(h).or_insert(r);
        }
        pages.push(PageFacts {
            slug: page.slug.clone(),
            title: page.title.clone(),
            nav: page.nav.clone(),
            section: page.section.clone(),
            order: page.order,
            window: page.window,
            frags,
        });
    }

    let blob_base = format!("{}/blob/main/", cfg.site.repo_url.trim_end_matches('/'));
    let mut lenient = HashSet::new();
    for entry in &cfg.snapshots {
        let frags = snapshot::build(entry, releases, &args.source_repo, &blob_base, &slugs, hash)
            .with_context(|| format!("snapshot page {}", entry.slug))?;
        let mut facts = Vec::new();
        for f in frags {
            facts.push(FragFacts {
                order: f.order,
                blob: f.hash.clone(),
                heading: f.rendered.heading.clone(),
                window: f.window,
                added_in: None,
            });
            blobs.entry(f.hash).or_insert(f.rendered);
        }
        pages.push(PageFacts {
            slug: entry.slug.clone(),
            title: entry.title.clone(),
            nav: entry.title.clone(),
            section: entry.section.clone(),
            order: entry.order,
            window: Window {
                from: Some(version::Version::parse(&entry.since)?),
                to: None,
            },
            frags: facts,
        });
        lenient.insert(entry.slug.clone());
    }
    Ok(Sources {
        pages,
        blobs,
        lenient,
    })
}

fn build(args: &Args) -> Result<()> {
    let cfg = Config::load(&args.root.join("site.toml"))?;
    let releases = cfg.releases()?;
    let latest = releases
        .iter()
        .rev()
        .find(|r| r.released)
        .context("no released version in site.toml")?;
    let sources = load_sources(&cfg, &releases, args)?;

    if args.out.exists() {
        fs::remove_dir_all(&args.out)
            .with_context(|| format!("clearing {}", args.out.display()))?;
    }
    fs::create_dir_all(&args.out)?;

    // The index.
    let graph = DocsGraph::create(&args.out.join("docs.graph"))?;
    graph.write_versions(&releases)?;
    for page in &sources.pages {
        graph.write_page(page, 0)?;
    }
    graph.checkpoint()?;

    // Every page at every version, read back from the graph.
    let mut nav_by_tag: HashMap<String, Vec<NavEntry>> = HashMap::new();
    for r in &releases {
        let nav = graph.nav_at(r.version)?;
        if !nav.iter().any(|e| e.slug == cfg.site.home) {
            bail!("home page `{}` does not exist in {}", cfg.site.home, r.tag);
        }
        nav_by_tag.insert(r.tag.clone(), nav);
    }
    let blob_html: HashMap<String, String> = sources
        .blobs
        .iter()
        .map(|(h, r)| (h.clone(), r.html.clone()))
        .collect();

    let mut errors = Vec::new();
    let mut sitemap = Vec::new();
    let mut page_count = 0;
    for r in &releases {
        let nav = &nav_by_tag[&r.tag];
        let mut frags_by_slug = HashMap::new();
        for e in nav {
            frags_by_slug.insert(e.slug.clone(), graph.frags_at(&e.slug, r.version)?);
        }
        let ids_of = |slug: &str| -> HashSet<&str> {
            frags_by_slug[slug]
                .iter()
                .flat_map(|f| sources.blobs[&f.blob].ids.iter().map(String::as_str))
                .collect()
        };
        for e in nav {
            check_links(e, r, &frags_by_slug, &sources, &ids_of, &mut errors);
        }

        let dirs: Vec<&str> = if r.tag == latest.tag {
            vec![r.tag.as_str(), "latest"]
        } else {
            vec![r.tag.as_str()]
        };
        for dir in dirs {
            for e in nav {
                let canonical = if dir == "latest" || r.tag == latest.tag {
                    format!("{}latest/{}/", cfg.site.url, e.slug)
                } else {
                    format!("{}{}/{}/", cfg.site.url, r.tag, e.slug)
                };
                let ctx = PageCtx {
                    site: &cfg.site,
                    releases: &releases,
                    release: r,
                    latest,
                    root: "../../",
                    dir,
                    pages_by_tag: &nav_by_tag,
                    canonical: Some(canonical.clone()),
                };
                let html = site::doc_page(&ctx, nav, e, &frags_by_slug[&e.slug], &blob_html);
                write(&args.out.join(dir).join(&e.slug).join("index.html"), html)?;
                page_count += 1;
                if dir == "latest" || r.tag != latest.tag {
                    sitemap.push(canonical);
                }
            }
            write(
                &args.out.join(dir).join("index.html"),
                site::redirect(&format!("{}/", cfg.site.home)),
            )?;
        }
    }
    if !errors.is_empty() {
        for e in &errors {
            eprintln!("error: {e}");
        }
        bail!("{} broken link(s)", errors.len());
    }

    for (h, html) in &blob_html {
        write(&args.out.join("blobs").join(format!("{h}.html")), html)?;
    }

    let tool_ctx = PageCtx {
        site: &cfg.site,
        releases: &releases,
        release: latest,
        latest,
        root: "../",
        dir: "latest",
        pages_by_tag: &nav_by_tag,
        canonical: None,
    };
    let latest_nav = &nav_by_tag[&latest.tag];
    write(
        &args.out.join("diff/index.html"),
        site::diff_page(&tool_ctx, latest_nav),
    )?;
    write(
        &args.out.join("console/index.html"),
        site::console_page(&tool_ctx, latest_nav),
    )?;
    write(&args.out.join("index.html"), site::redirect("latest/"))?;
    write(&args.out.join("404.html"), site::not_found(&cfg.site))?;
    write(&args.out.join(".nojekyll"), "")?;
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for url in &sitemap {
        xml.push_str(&format!("<url><loc>{}</loc></url>\n", render::escape(url)));
    }
    xml.push_str("</urlset>\n");
    write(&args.out.join("sitemap.xml"), xml)?;
    copy_dir(&args.root.join("assets"), &args.out.join("assets"))?;

    // GitHub Pages does not compress an unknown file type; the browser inflates this copy
    // with DecompressionStream and falls back to docs.graph.
    let raw = fs::read(args.out.join("docs.graph"))?;
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    std::io::Write::write_all(&mut gz, &raw)?;
    write(&args.out.join("docs.graph.gz"), gz.finish()?)?;
    let graph_size = raw.len();
    eprintln!(
        "built {} versions, {} pages, {} fragments in {} blobs, docs.graph {} KiB",
        releases.len(),
        page_count,
        sources.pages.iter().map(|p| p.frags.len()).sum::<usize>(),
        blob_html.len(),
        graph_size / 1024
    );
    Ok(())
}

fn check_links<'a>(
    page: &NavEntry,
    r: &Release,
    frags_by_slug: &HashMap<String, Vec<graph::FragAt>>,
    sources: &'a Sources,
    ids_of: &dyn Fn(&str) -> HashSet<&'a str>,
    errors: &mut Vec<String>,
) {
    let lenient = sources.lenient.contains(&page.slug);
    for f in &frags_by_slug[&page.slug] {
        for Link {
            page: target,
            anchor,
        } in &sources.blobs[&f.blob].links
        {
            let target_slug = target.as_deref().unwrap_or(&page.slug);
            let problem = if !frags_by_slug.contains_key(target_slug) {
                Some(format!("page `{target_slug}` does not exist"))
            } else {
                anchor
                    .as_deref()
                    .filter(|a| !ids_of(target_slug).contains(a))
                    .map(|a| format!("`{target_slug}#{a}` names no heading"))
            };
            if let Some(p) = problem {
                let msg = format!("{} at {}: {p}", page.slug, r.tag);
                if lenient {
                    eprintln!("warning: {msg}");
                } else {
                    errors.push(msg);
                }
            }
        }
    }
}
