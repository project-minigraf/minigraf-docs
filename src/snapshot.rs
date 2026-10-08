//! Snapshot pages: a file from the source repository, read at each version's git ref.
//!
//! Every version's text is split and rendered on its own. The fragment sequences are then
//! merged into one supersequence (by longest common subsequence on the rendered HTML), so a
//! fragment that stays put across versions is one entity with one window, and its position
//! is stable for the diff view.

use crate::config::{Release, SnapshotEntry};
use crate::render::{IdAllocator, LinkMode, Rendered, render};
use crate::source::split_plain;
use crate::version::{Version, Window};
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

pub struct SnapshotFrag {
    pub order: i64,
    pub hash: String,
    pub rendered: Rendered,
    pub window: Window,
}

fn git_show(repo: &Path, git_ref: &str, path: &str) -> Result<Option<String>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("show")
        .arg(format!("{git_ref}:{path}"))
        .output()
        .context("running git")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        if err.contains("does not exist") || err.contains("exists on disk, but not in") {
            return Ok(None);
        }
        anyhow::bail!("git show {git_ref}:{path} failed: {}", err.trim());
    }
    Ok(Some(
        String::from_utf8(out.stdout).context("file is not UTF-8")?,
    ))
}

/// Drops the leading `# Title` line; the page template supplies the title.
fn strip_title(text: &str) -> &str {
    let t = text.trim_start();
    if t.starts_with("# ") {
        t.split_once('\n').map(|(_, rest)| rest).unwrap_or("")
    } else {
        t
    }
}

pub fn build(
    entry: &SnapshotEntry,
    releases: &[Release],
    repo: &Path,
    blob_base: &str,
    slugs: &BTreeSet<String>,
    hash: impl Fn(&str) -> String,
) -> Result<Vec<SnapshotFrag>> {
    let since = Version::parse(&entry.since)?;
    let dir = match entry.path.rfind('/') {
        Some(i) => &entry.path[..=i],
        None => "",
    };
    let mode = LinkMode::Snapshot { blob_base, dir };

    // Per version: the rendered fragments, or None where the file does not exist.
    let mut per_version: Vec<Option<Vec<(String, Rendered)>>> = Vec::new();
    for r in releases {
        if r.version < since {
            per_version.push(None);
            continue;
        }
        let Some(text) = git_show(repo, &r.git_ref, &entry.path)? else {
            per_version.push(None);
            continue;
        };
        let mut ids = IdAllocator::default();
        let mut seq = Vec::new();
        for md in split_plain(strip_title(&text), entry.split)? {
            let rendered = render(&md, slugs, &mut ids, Window::ALL, entry.scoped_ids, &mode)
                .with_context(|| format!("{} at {}", entry.path, r.tag))?;
            seq.push((hash(&rendered.html), rendered));
        }
        per_version.push(Some(seq));
    }

    // Merge into one supersequence. Each merged item records the versions it appears in.
    let mut merged: Vec<(String, Rendered, Vec<usize>)> = Vec::new();
    for (vi, seq) in per_version.iter().enumerate() {
        let Some(seq) = seq else { continue };
        let a_owned: Vec<String> = merged.iter().map(|m| m.0.clone()).collect();
        let a: Vec<&str> = a_owned.iter().map(String::as_str).collect();
        let b: Vec<&str> = seq.iter().map(|s| s.0.as_str()).collect();
        let pairs = lcs(&a, &b);
        let mut next = Vec::with_capacity(merged.len() + seq.len());
        let mut old = std::mem::take(&mut merged)
            .into_iter()
            .enumerate()
            .peekable();
        let mut bi = 0;
        for (ai, bj) in pairs
            .iter()
            .copied()
            .chain(std::iter::once((a.len(), b.len())))
        {
            while let Some((i, _)) = old.peek() {
                if *i >= ai {
                    break;
                }
                next.push(old.next().unwrap().1);
            }
            while bi < bj {
                let (h, r) = &seq[bi];
                next.push((h.clone(), r.clone(), vec![vi]));
                bi += 1;
            }
            if ai < a.len() {
                let mut item = old.next().unwrap().1;
                item.2.push(vi);
                next.push(item);
                bi += 1;
            }
        }
        merged = next;
    }

    let last = releases.len() - 1;
    let mut out = Vec::new();
    for (order, (h, rendered, vis)) in merged.into_iter().enumerate() {
        // Consecutive runs of versions become windows.
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for vi in vis {
            match runs.last_mut() {
                Some((_, end)) if *end + 1 == vi => *end = vi,
                _ => runs.push((vi, vi)),
            }
        }
        for (start, end) in runs {
            out.push(SnapshotFrag {
                order: order as i64,
                hash: h.clone(),
                rendered: rendered.clone(),
                window: Window {
                    from: Some(releases[start].version),
                    to: (end < last).then(|| releases[end + 1].version),
                },
            });
        }
    }
    Ok(out)
}

/// Longest common subsequence as index pairs, in order.
fn lcs(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut t = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if a[i] == b[j] {
                t[i + 1][j + 1] + 1
            } else {
                t[i + 1][j].max(t[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((i, j));
            i += 1;
            j += 1;
        } else if t[i + 1][j] >= t[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcs_pairs() {
        assert_eq!(
            lcs(&["a", "b", "c"], &["x", "a", "c"]),
            vec![(0, 1), (2, 2)]
        );
        assert_eq!(lcs(&[], &["a"]), vec![]);
    }

    #[test]
    fn strips_title() {
        assert_eq!(strip_title("# Changelog\n\nintro\n"), "\nintro\n");
        assert_eq!(strip_title("## A\n"), "## A\n");
    }
}
