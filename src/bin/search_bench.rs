//! Prototype for #8: compares two search-index layouts in `docs.graph`.
//!
//! ```text
//! search_bench <dist dir> <out dir>
//! ```
//!
//! Reads the built site's `docs.graph` and `blobs/`, writes `<out>/a/docs.graph` (one entity per
//! (term, blob) posting) and `<out>/b/docs.graph` (one entity per term chunk, blob list in a
//! string), and prints sizes, build times and native query latencies.

use flate2::{Compression, write::GzEncoder};
use minigraf::{Minigraf, QueryResult, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "can", "do", "does", "for", "from",
    "has", "have", "if", "in", "into", "is", "it", "its", "no", "not", "of", "on", "one", "or",
    "so", "than", "that", "the", "their", "then", "there", "these", "this", "to", "was", "we",
    "were", "when", "which", "while", "who", "will", "with", "you", "your",
];

/// Visible text of an HTML fragment, and the text of its headings.
fn text(html: &str) -> (String, String) {
    let mut body = String::new();
    let mut heading = String::new();
    let mut in_tag = false;
    let mut tag = String::new();
    let mut in_heading = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let t = tag.to_ascii_lowercase();
                if t.starts_with("h2") || t.starts_with("h3") || t.starts_with("h4") {
                    in_heading = true;
                } else if t.starts_with("/h2") || t.starts_with("/h3") || t.starts_with("/h4") {
                    in_heading = false;
                }
                body.push(' ');
            }
            c if in_tag => tag.push(c),
            c => {
                body.push(c);
                if in_heading {
                    heading.push(c);
                }
            }
        }
    }
    let unescape = |s: String| {
        s.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&")
    };
    (unescape(body), unescape(heading))
}

/// Lower-cased alphanumeric runs, plus hyphenated compounds (`valid-at`, `stg-028`).
fn tokens(s: &str) -> Vec<String> {
    let lower = s.to_lowercase();
    let mut out = Vec::new();
    for compound in lower.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
        let parts: Vec<&str> = compound.split('-').filter(|p| !p.is_empty()).collect();
        if parts.len() > 1 {
            out.push(parts.join("-"));
        }
        for p in parts {
            if p.len() >= 2 && !STOPWORDS.contains(&p) {
                out.push(p.to_string());
            }
        }
    }
    out
}

fn edn_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// term → blob → weight (occurrences, +10 per heading occurrence).
fn postings(dist: &Path) -> BTreeMap<String, BTreeMap<String, i64>> {
    let mut index: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
    for entry in fs::read_dir(dist.join("blobs")).unwrap() {
        let path = entry.unwrap().path();
        let blob = path.file_stem().unwrap().to_string_lossy().to_string();
        let (body, heading) = text(&fs::read_to_string(&path).unwrap());
        for t in tokens(&body) {
            *index.entry(t).or_default().entry(blob.clone()).or_default() += 1;
        }
        for t in tokens(&heading) {
            *index.entry(t).or_default().entry(blob.clone()).or_default() += 10;
        }
    }
    index
}

fn fresh_copy(dist: &Path, out: &Path) -> Minigraf {
    fs::create_dir_all(out).unwrap();
    let path = out.join("docs.graph");
    let _ = fs::remove_file(out.join("docs.graph.wal"));
    fs::copy(dist.join("docs.graph"), &path).unwrap();
    Minigraf::open(&path).unwrap()
}

const ALL: &str = "{:valid-from \"2000-01-01T00:00:00Z\"}";

fn transact_chunked(db: &Minigraf, facts: &[String], per_tx: usize) {
    for chunk in facts.chunks(per_tx) {
        db.execute(&format!("(transact [{}])", chunk.join(" ")))
            .unwrap();
    }
}

/// Layout A: `:term/text` per term; one posting entity per (term, blob).
fn build_a(db: &Minigraf, index: &BTreeMap<String, BTreeMap<String, i64>>) {
    let mut facts = Vec::new();
    let mut n = 0usize;
    for (k, (term, blobs)) in index.iter().enumerate() {
        facts.push(format!("[:s/t{k} :term/text {} {ALL}]", edn_str(term)));
        for (blob, w) in blobs {
            facts.push(format!(
                "[:s/p{n} :post/term {t} {ALL}] [:s/p{n} :post/blob {b} {ALL}] \
                 [:s/p{n} :post/weight {w} {ALL}]",
                t = edn_str(term),
                b = edn_str(blob)
            ));
            n += 1;
        }
    }
    transact_chunked(db, &facts, 2000);
}

/// Layout B: one entity per term chunk, `:term/blobs` = "blob:weight blob:weight …".
const CHUNK_BYTES: usize = 3000;

fn build_b(db: &Minigraf, index: &BTreeMap<String, BTreeMap<String, i64>>) {
    let mut facts = Vec::new();
    let mut n = 0usize;
    for (term, blobs) in index {
        let mut cur = String::new();
        let mut flush = |cur: &mut String, facts: &mut Vec<String>| {
            facts.push(format!(
                "[:s/c{n} :term/text {} {ALL}] [:s/c{n} :term/blobs {} {ALL}]",
                edn_str(term),
                edn_str(cur.trim_end())
            ));
            n += 1;
            cur.clear();
        };
        for (blob, w) in blobs {
            if cur.len() + blob.len() + 8 > CHUNK_BYTES {
                flush(&mut cur, &mut facts);
            }
            cur.push_str(&format!("{blob}:{w} "));
        }
        flush(&mut cur, &mut facts);
    }
    transact_chunked(db, &facts, 2000);
}

fn rows(db: &Minigraf, q: &str) -> Vec<Vec<Value>> {
    match db.execute(q).unwrap() {
        QueryResult::QueryResults { results, .. } => results,
        _ => panic!("not a query"),
    }
}

fn s(v: &Value) -> &str {
    match v {
        Value::String(s) => s,
        _ => panic!("expected string"),
    }
}

fn time<T>(f: impl Fn() -> T) -> (f64, T) {
    let r = f();
    let reps = 5;
    let t = Instant::now();
    for _ in 0..reps {
        f();
    }
    (t.elapsed().as_secs_f64() * 1000.0 / reps as f64, r)
}

const V3: &str = "2003-01-01T00:00:00Z";

fn term_clause(var: &str, term: &str, prefix: bool) -> String {
    if prefix {
        format!(
            "[?t{var} :term/text ?x{var}] [(starts-with? ?x{var} {})]",
            edn_str(term)
        )
    } else {
        format!(
            "[?t{var} :term/text ?x{var}] [(= ?x{var} {})]",
            edn_str(term)
        )
    }
}

/// A: one query; postings joined to the fragments at the version.
fn query_a(db: &Minigraf, terms: &[(&str, bool)]) -> usize {
    let mut wh = String::new();
    for (i, (t, prefix)) in terms.iter().enumerate() {
        if *prefix {
            wh.push_str(&term_clause(&i.to_string(), t, true));
            wh.push_str(&format!(" [?p{i} :post/term ?x{i}]"));
        } else {
            wh.push_str(&format!("[?p{i} :post/term {}]", edn_str(t)));
        }
        wh.push_str(&format!(
            " [?p{i} :post/blob ?blob] [?p{i} :post/weight ?w{i}] "
        ));
    }
    let q = format!(
        "(query [:find ?slug ?blob ?w0 :valid-at \"{V3}\" :where {wh} \
         [?f :frag/blob ?blob] [?f :frag/page ?slug]])"
    );
    rows(db, &q).len()
}

/// A2: layout A, postings per term queried alone, joined to the fragments here.
fn query_a2(db: &Minigraf, terms: &[(&str, bool)]) -> usize {
    let mut sets: Vec<HashSet<String>> = Vec::new();
    for (t, prefix) in terms {
        let q = if *prefix {
            format!(
                "(query [:find ?blob ?w :valid-at \"{V3}\" :where [?t :term/text ?x] \
                 [(starts-with? ?x {})] [?p :post/term ?x] [?p :post/blob ?blob] [?p :post/weight ?w]])",
                edn_str(t)
            )
        } else {
            format!(
                "(query [:find ?blob ?w :valid-at \"{V3}\" :where [?p :post/term {}] \
                 [?p :post/blob ?blob] [?p :post/weight ?w]])",
                edn_str(t)
            )
        };
        sets.push(rows(db, &q).iter().map(|r| s(&r[0]).to_string()).collect());
    }
    let frags = rows(db, &frags_q());
    frags
        .iter()
        .filter(|r| sets.iter().all(|set| set.contains(s(&r[1]))))
        .count()
}

fn frags_q() -> String {
    format!(
        "(query [:find ?slug ?blob :valid-at \"{V3}\" :where [?f :frag/blob ?blob] \
         [?f :frag/page ?slug]])"
    )
}

/// B2: layout B with blobs numbered (`:blob/n`), lists as "n:weight n:weight …".
fn build_b2(db: &Minigraf, index: &BTreeMap<String, BTreeMap<String, i64>>) {
    let blobs: std::collections::BTreeSet<&String> =
        index.values().flat_map(|b| b.keys()).collect();
    let num: HashMap<&String, usize> = blobs.iter().enumerate().map(|(i, b)| (*b, i)).collect();
    let mut facts: Vec<String> = blobs
        .iter()
        .enumerate()
        .map(|(i, b)| {
            format!(
                "[:s/b{i} :blob/hash {} {ALL}] [:s/b{i} :blob/n {i} {ALL}]",
                edn_str(b)
            )
        })
        .collect();
    let mut n = 0usize;
    for (term, bl) in index {
        let mut cur = String::new();
        let mut flush = |cur: &mut String, facts: &mut Vec<String>| {
            facts.push(format!(
                "[:s/c{n} :term/text {} {ALL}] [:s/c{n} :term/blobs {} {ALL}]",
                edn_str(term),
                edn_str(cur.trim_end())
            ));
            n += 1;
            cur.clear();
        };
        for (blob, w) in bl {
            if cur.len() + 16 > CHUNK_BYTES {
                flush(&mut cur, &mut facts);
            }
            cur.push_str(&format!("{}:{w} ", num[blob]));
        }
        flush(&mut cur, &mut facts);
    }
    transact_chunked(db, &facts, 2000);
}

fn query_b2(db: &Minigraf, terms: &[(&str, bool)]) -> usize {
    let mut sets: Vec<HashSet<i64>> = Vec::new();
    for (t, prefix) in terms {
        let q = if *prefix {
            format!(
                "(query [:find ?x ?blobs :valid-at \"{V3}\" :where [?t :term/text ?x] \
                 [(starts-with? ?x {})] [?t :term/blobs ?blobs]])",
                edn_str(t)
            )
        } else {
            format!(
                "(query [:find ?t ?blobs :valid-at \"{V3}\" :where [?t :term/text {}] \
                 [?t :term/blobs ?blobs]])",
                edn_str(t)
            )
        };
        let mut set = HashSet::new();
        for row in rows(db, &q) {
            for e in s(&row[1]).split(' ') {
                set.insert(e.split_once(':').unwrap().0.parse::<i64>().unwrap());
            }
        }
        sets.push(set);
    }
    let frags = rows(
        db,
        &format!(
            "(query [:find ?slug ?n :valid-at \"{V3}\" :where [?f :frag/blob ?blob] \
             [?f :frag/page ?slug] [?b :blob/hash ?blob] [?b :blob/n ?n]])"
        ),
    );
    frags
        .iter()
        .filter(|r| match &r[1] {
            Value::Integer(n) => sets.iter().all(|set| set.contains(n)),
            _ => false,
        })
        .count()
}

/// B: one query per term for its blob lists, one for the fragments at the version; joined here
/// (in the browser, in JavaScript).
fn query_b(db: &Minigraf, terms: &[(&str, bool)]) -> usize {
    let mut sets: Vec<HashMap<String, i64>> = Vec::new();
    for (t, prefix) in terms {
        let q = if *prefix {
            format!(
                "(query [:find ?x ?blobs :valid-at \"{V3}\" :where [?t :term/text ?x] \
                 [(starts-with? ?x {})] [?t :term/blobs ?blobs]])",
                edn_str(t)
            )
        } else {
            format!(
                "(query [:find ?t ?blobs :valid-at \"{V3}\" :where [?t :term/text {}] \
                 [?t :term/blobs ?blobs]])",
                edn_str(t)
            )
        };
        let mut set = HashMap::new();
        for row in rows(db, &q) {
            for e in s(&row[1]).split(' ') {
                let (b, w) = e.split_once(':').unwrap();
                *set.entry(b.to_string()).or_default() += w.parse::<i64>().unwrap();
            }
        }
        sets.push(set);
    }
    let frags = rows(
        db,
        &format!(
            "(query [:find ?slug ?blob :valid-at \"{V3}\" :where [?f :frag/blob ?blob] \
             [?f :frag/page ?slug]])"
        ),
    );
    frags
        .iter()
        .filter(|r| sets.iter().all(|set| set.contains_key(s(&r[1]))))
        .count()
}

fn gz_len(path: &Path) -> usize {
    let mut e = GzEncoder::new(Vec::new(), Compression::best());
    e.write_all(&fs::read(path).unwrap()).unwrap();
    e.finish().unwrap().len()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dist = Path::new(&args[0]);
    let out = Path::new(&args[1]);

    let index = postings(dist);
    let pairs: usize = index.values().map(|b| b.len()).sum();
    let distinct: HashSet<&String> = index.values().flat_map(|b| b.keys()).collect();
    println!(
        "blobs {} terms {} postings {pairs}",
        distinct.len(),
        index.len()
    );
    let base = dist.join("docs.graph");
    println!(
        "baseline: {} bytes, gz {} bytes",
        fs::metadata(&base).unwrap().len(),
        gz_len(&base)
    );

    let queries: &[(&str, &[(&str, bool)])] = &[
        ("exact checkpoint", &[("checkpoint", false)]),
        ("exact valid-at", &[("valid-at", false)]),
        ("exact query (common)", &[("query", false)]),
        ("prefix recurs*", &[("recurs", true)]),
        ("prefix q* (wide)", &[("q", true)]),
        (
            "and recursive+rule",
            &[("recursive", false), ("rule", false)],
        ),
        ("and bitemp*+valid", &[("bitemp", true), ("valid", false)]),
    ];

    let only: Option<&str> = args.get(2).map(|s| s.as_str());
    for (name, build, query) in [
        (
            "A",
            build_a as fn(&Minigraf, &_),
            query_a as fn(&Minigraf, &[(&str, bool)]) -> usize,
        ),
        ("A2", build_a, query_a2),
        ("B", build_b, query_b),
        ("B2", build_b2, query_b2),
    ] {
        if only.is_some_and(|o| !o.split(',').any(|x| x == name)) {
            continue;
        }
        let dir = out.join(name.to_lowercase());
        let db = fresh_copy(dist, &dir);
        let t = Instant::now();
        build(&db, &index);
        db.checkpoint().unwrap();
        let build_ms = t.elapsed().as_millis();
        drop(db);
        let path = dir.join("docs.graph");
        println!(
            "\nlayout {name}: {} bytes, gz {} bytes, build {build_ms} ms",
            fs::metadata(&path).unwrap().len(),
            gz_len(&path)
        );
        let t = Instant::now();
        let db = Minigraf::open(&path).unwrap();
        println!("  open {} ms", t.elapsed().as_millis());
        for (qname, terms) in queries {
            let (ms, n) = time(|| query(&db, terms));
            println!("  {qname:<24} {n:>5} hits {ms:>9.2} ms");
        }
    }
}
