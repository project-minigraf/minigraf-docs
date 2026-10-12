//! The search index, `search-<build id>.graph`. See docs/specs/2026-10-12-search-design.md.
//!
//! One entity per term chunk holds the term and a string listing the blobs it occurs in with a
//! weight each. Blobs are version-free; the browser joins them with `docs.graph` at the version
//! being read. The build id names the file and is stored in both graphs, so the browser always
//! loads the search index built with its `docs.graph`.

use crate::graph::{edn_str, window_map};
use crate::render::Rendered;
use crate::tokenize::tokenize;
use crate::version::Window;
use anyhow::{Context, Result, bail};
use minigraf::{Minigraf, QueryResult, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;

/// Longest `:term/blobs` string. Format v7 rejects a serialized fact over 4,080 bytes
/// (`MAX_FACT_BYTES`); the rest of the fact takes well under 1,000.
pub const CHUNK_BYTES: usize = 3000;

/// Extra weight for each occurrence of a term in a heading.
const HEADING_WEIGHT: i64 = 10;

/// Facts per transaction when writing the index.
const FACTS_PER_TX: usize = 2000;

pub struct SearchIndex {
    /// term → blob → weight.
    terms: BTreeMap<String, BTreeMap<String, i64>>,
    /// blob → id of the heading it starts with, or "".
    anchors: BTreeMap<String, String>,
}

impl SearchIndex {
    pub fn build(blobs: &HashMap<String, Rendered>) -> Self {
        let mut terms: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
        let mut anchors = BTreeMap::new();
        for (hash, r) in blobs {
            for t in tokenize(&r.text) {
                *terms.entry(t).or_default().entry(hash.clone()).or_default() += 1;
            }
            for t in tokenize(&r.heading_text) {
                *terms.entry(t).or_default().entry(hash.clone()).or_default() += HEADING_WEIGHT;
            }
            let anchor = match (&r.heading, r.ids.first()) {
                (Some(_), Some(id)) => id.clone(),
                _ => String::new(),
            };
            anchors.insert(hash.clone(), anchor);
        }
        SearchIndex { terms, anchors }
    }

    fn entries(blobs: &BTreeMap<String, i64>) -> impl Iterator<Item = String> + '_ {
        blobs.iter().map(|(b, w)| format!("{b}:{w}"))
    }

    /// (term, list) pairs, each list at most `CHUNK_BYTES` long.
    pub fn chunks(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (term, blobs) in &self.terms {
            let mut cur = String::new();
            for e in Self::entries(blobs) {
                if !cur.is_empty() && cur.len() + 1 + e.len() > CHUNK_BYTES {
                    out.push((term.clone(), std::mem::take(&mut cur)));
                }
                if !cur.is_empty() {
                    cur.push(' ');
                }
                cur.push_str(&e);
            }
            out.push((term.clone(), cur));
        }
        out
    }

    /// First 16 hex digits of the SHA-256 of the index's canonical text: `<term>\t<list>` per
    /// term, then `<blob>\t<anchor>` per blob, each line ending in `\n`.
    pub fn build_id(&self) -> String {
        let mut h = Sha256::new();
        for (term, blobs) in &self.terms {
            let list: Vec<String> = Self::entries(blobs).collect();
            h.update(format!("{term}\t{}\n", list.join(" ")));
        }
        for (blob, anchor) in &self.anchors {
            h.update(format!("{blob}\t{anchor}\n"));
        }
        h.finalize()[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Writes the index to a new file at `path`.
    pub fn write(&self, path: &Path, build_id: &str) -> Result<()> {
        for p in [path.to_path_buf(), path.with_extension("graph.wal")] {
            if p.exists() {
                fs::remove_file(&p).with_context(|| format!("removing {}", p.display()))?;
            }
        }
        let db = Minigraf::open(path).with_context(|| format!("opening {}", path.display()))?;
        let all = window_map(Window::ALL);
        let mut facts = vec![format!(
            "[:search/build :build/id {} {all}]",
            edn_str(build_id)
        )];
        for (n, (blob, anchor)) in self.anchors.iter().enumerate() {
            facts.push(format!(
                "[:search/b{n} :blob/hash {} {all}] [:search/b{n} :blob/anchor {} {all}]",
                edn_str(blob),
                edn_str(anchor)
            ));
        }
        for (n, (term, list)) in self.chunks().into_iter().enumerate() {
            facts.push(format!(
                "[:search/c{n} :term/text {} {all}] [:search/c{n} :term/blobs {} {all}]",
                edn_str(&term),
                edn_str(&list)
            ));
        }
        // Two facts per entry; one entity's facts always share a transaction.
        for batch in facts.chunks(FACTS_PER_TX / 2) {
            db.execute(&format!("(transact [{}])", batch.join("\n")))
                .with_context(|| format!("writing {}", path.display()))?;
        }
        db.checkpoint()
            .with_context(|| format!("checkpointing {}", path.display()))
    }
}

fn rows(db: &Minigraf, q: &str) -> Result<Vec<Vec<Value>>> {
    match db.execute(q).with_context(|| format!("executing: {q}"))? {
        QueryResult::QueryResults { results, .. } => Ok(results),
        _ => bail!("expected query results"),
    }
}

fn string(v: &Value) -> Result<&str> {
    match v {
        Value::String(s) => Ok(s),
        _ => bail!("expected a string in query results"),
    }
}

/// Reopens a written index and checks it belongs with `docs.graph`: the same build id, the same
/// set of blobs as `docs.graph`'s `:frag/blob` values, and term lists naming only those blobs.
pub fn verify(path: &Path, build_id: &str, frag_blobs: &BTreeSet<String>) -> Result<()> {
    let db = Minigraf::open(path).with_context(|| format!("opening {}", path.display()))?;
    let ids: Vec<String> = rows(
        &db,
        "(query [:find ?id :any-valid-time :where [?b :build/id ?id]])",
    )?
    .iter()
    .map(|r| string(&r[0]).map(str::to_string))
    .collect::<Result<_>>()?;
    if ids != [build_id] {
        bail!(
            "{}: build id is {ids:?}, expected {build_id}",
            path.display()
        );
    }
    let blobs: BTreeSet<String> = rows(
        &db,
        "(query [:find ?h :any-valid-time :where [?b :blob/hash ?h]])",
    )?
    .iter()
    .map(|r| string(&r[0]).map(str::to_string))
    .collect::<Result<_>>()?;
    if &blobs != frag_blobs {
        let missing: Vec<_> = frag_blobs.difference(&blobs).collect();
        let extra: Vec<_> = blobs.difference(frag_blobs).collect();
        bail!(
            "{}: blobs differ from docs.graph (missing {missing:?}, not in docs.graph {extra:?})",
            path.display()
        );
    }
    for r in rows(
        &db,
        "(query [:find ?t ?l :any-valid-time :where [?t :term/blobs ?l]])",
    )? {
        for e in string(&r[1])?.split(' ') {
            let blob = e.split_once(':').map_or(e, |(b, _)| b);
            if !blobs.contains(blob) {
                bail!("{}: a term list names unknown blob {blob}", path.display());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rendered(text: &str, heading: Option<&str>) -> Rendered {
        Rendered {
            html: String::new(),
            heading: heading.map(str::to_string),
            ids: heading.map(|h| vec![h.to_lowercase()]).unwrap_or_default(),
            text: format!("{} {text}", heading.unwrap_or("")),
            heading_text: heading.unwrap_or("").to_string(),
            links: Default::default(),
        }
    }

    fn corpus() -> HashMap<String, Rendered> {
        HashMap::from([
            (
                "aaaa".to_string(),
                rendered("query query rules", Some("Negation")),
            ),
            ("bbbb".to_string(), rendered("a query of the graph", None)),
        ])
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("minigraf-docs-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("search.graph")
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn weights_and_anchors() {
        let idx = SearchIndex::build(&corpus());
        assert_eq!(idx.terms["query"]["aaaa"], 2);
        assert_eq!(idx.terms["query"]["bbbb"], 1);
        assert_eq!(idx.terms["negation"]["aaaa"], 1 + HEADING_WEIGHT);
        assert!(!idx.terms.contains_key("the"));
        assert_eq!(idx.anchors["aaaa"], "negation");
        assert_eq!(idx.anchors["bbbb"], "");
        let chunks = idx.chunks();
        assert!(chunks.contains(&("query".to_string(), "aaaa:2 bbbb:1".to_string())));
    }

    #[test]
    fn long_lists_split_into_chunks() {
        let blobs: HashMap<String, Rendered> = (0..400)
            .map(|i| (format!("{i:016x}"), rendered("common", None)))
            .collect();
        let idx = SearchIndex::build(&blobs);
        let chunks: Vec<_> = idx
            .chunks()
            .into_iter()
            .filter(|(t, _)| t == "common")
            .collect();
        assert!(chunks.len() > 1, "expected several chunks");
        let mut seen = BTreeSet::new();
        for (_, list) in &chunks {
            assert!(list.len() <= CHUNK_BYTES);
            for e in list.split(' ') {
                assert!(
                    seen.insert(e.split_once(':').unwrap().0.to_string()),
                    "repeat"
                );
            }
        }
        assert_eq!(seen.len(), 400);
    }

    #[test]
    fn a_full_chunk_fits_in_a_file_backed_graph() {
        let blobs: HashMap<String, Rendered> = (0..400)
            .map(|i| (format!("{i:016x}"), rendered("common", None)))
            .collect();
        let idx = SearchIndex::build(&blobs);
        let longest = idx.chunks().iter().map(|(_, l)| l.len()).max().unwrap();
        assert!(longest > CHUNK_BYTES - 25, "test should hold a full chunk");
        let path = temp("full-chunk");
        let id = idx.build_id();
        idx.write(&path, &id).unwrap();
        verify(&path, &id, &blobs.keys().cloned().collect()).unwrap();
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn build_id_is_stable_and_follows_content() {
        let a = SearchIndex::build(&corpus()).build_id();
        assert_eq!(a.len(), 16);
        assert_eq!(a, SearchIndex::build(&corpus()).build_id());
        let mut changed = corpus();
        changed.insert("bbbb".into(), rendered("a query of the tree", None));
        assert_ne!(a, SearchIndex::build(&changed).build_id());
    }

    #[test]
    fn written_index_round_trips_and_verify_catches_mismatches() {
        let idx = SearchIndex::build(&corpus());
        let id = idx.build_id();
        let path = temp("round-trip");
        idx.write(&path, &id).unwrap();
        verify(&path, &id, &set(&["aaaa", "bbbb"])).unwrap();

        let db = Minigraf::open(&path).unwrap();
        let r = rows(
            &db,
            "(query [:find ?l :any-valid-time :where [?t :term/text \"query\"] [?t :term/blobs ?l]])",
        )
        .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(string(&r[0][0]).unwrap(), "aaaa:2 bbbb:1");
        let r = rows(
            &db,
            "(query [:find ?a :any-valid-time :where [?b :blob/hash \"aaaa\"] [?b :blob/anchor ?a]])",
        )
        .unwrap();
        assert_eq!(string(&r[0][0]).unwrap(), "negation");
        drop(db);

        assert!(verify(&path, "0000000000000000", &set(&["aaaa", "bbbb"])).is_err());
        assert!(verify(&path, &id, &set(&["aaaa"])).is_err());
        assert!(verify(&path, &id, &set(&["aaaa", "bbbb", "cccc"])).is_err());

        let db = Minigraf::open(&path).unwrap();
        db.execute(
            "(transact [[:search/x :term/text \"stray\"] [:search/x :term/blobs \"zzzz:1\"]])",
        )
        .unwrap();
        drop(db);
        assert!(verify(&path, &id, &set(&["aaaa", "bbbb"])).is_err());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
