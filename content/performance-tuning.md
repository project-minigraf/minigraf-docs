---
title: Performance Tuning
nav: Performance tuning
section: Reference
order: 30
---
This page covers what affects Minigraf's speed, which configuration knobs exist, and how to write
queries that perform well. Organized cost-model first — the knobs only make sense once you
understand where time is spent.

---

## Cost Model

<!-- @until v3.0.0 -->
| Operation | Cost | Notes |
|---|---|---|
| Insert / retract (any DB size) | O(1) | WAL append; independent of `.graph` size |
| Query — bound entity or attribute | O(k) | Selective index fetch; k = facts for that entity/attr |
| Query — no bound entity/attr | O(facts) | Full scan + in-memory filter |
| Expression predicate `[(> ?x N)]` | O(N), early | Pushed down at first bound variable |
| `not` / `not-join` | O(N²) worst case | Inner loop re-scans per binding |
| `or` / `or-join` (mid-query) | O(N²) worst case | Branch expansion over full binding set |
| `or` in a rule body | O(N) | Rule starts from empty binding — no re-scan |
| Recursive rules | Super-linear | Semi-naive; deep chains are expensive |
| Window functions | O(N log N) | Sort pass over result set |
| Aggregates (`count`, `sum`, `min`, `max`) | O(N) | Single pass |
| `(sum :with ?x)` cross-product join | O(N²) | No hash-join; avoid on large datasets |
| Open (file-backed) | O(facts) | Page-cache warming; B+tree roots loaded lazily |
| Checkpoint | O(facts) | WAL flush + B+tree rebuild across all 4 indexes |
<!-- @end -->
<!-- @since v3.0.0 -->
| Operation | Cost | Notes |
|---|---|---|
| Insert / retract (any DB size) | O(1) | WAL append; independent of `.graph` size |
| Query — bound entity or attribute | O(k) | Selective index fetch; k = facts for that entity/attr |
| Query — no bound entity/attr | O(facts) | Full scan + in-memory filter |
| Expression predicate `[(> ?x N)]` | O(N), early | Pushed down at first bound variable |
| `not` / `not-join` | O(N²) worst case | Inner loop re-scans per binding |
| `or` / `or-join` (mid-query) | O(N²) worst case | Branch expansion over full binding set |
| `or` in a rule body | O(N) | Rule starts from empty binding — no re-scan |
| Recursive rules | Super-linear | Semi-naive; deep chains are expensive |
| Window functions | O(N log N) | Sort pass over result set |
| Aggregates (`count`, `sum`, `min`, `max`) | O(N) | Single pass |
| `(sum :with ?x)` cross-product join | O(N²) | No hash-join; avoid on large datasets |
| Open (file-backed) | O(WAL) | Reads the newest valid meta page and replays the WAL; never rebuilds an index |
| Checkpoint | O(change) | Copy-on-write: rewrites only the touched leaves and their paths, about 3 ms after one new fact at 10k–100k facts |
<!-- @end -->

**Selective fetch threshold:** the engine counts distinct bound entities + bound attributes across all
patterns in the query. If the count is 1–4, it uses index-backed fetches instead of a full scan;
beyond 4, a full scan is cheaper and the engine falls back to it automatically.

---

## Configuration Knobs

```rust
let db = OpenOptions::new()
    .page_cache_size(1024)          // 1024 × 4KB = 4MB
    .wal_checkpoint_threshold(500)  // checkpoint every 500 WAL entries
    .max_derived_facts(50_000)      // recursive rule safety ceiling
    .max_results(500_000)           // result set safety ceiling
    .synchronous(SyncMode::Normal)  // skip per-write fsync; checkpoint stays durable
    .path("my.graph")
    .open()?;
```

| Option | Default | Unit |
|---|---|---|
| `page_cache_size` | 256 | pages (1 page = 4KB) |
| `wal_checkpoint_threshold` | 1000 | WAL entries |
| `max_derived_facts` | 1,000,000 | derived facts per rule iteration |
| `max_results` | 1,000,000 | total query results |
| `synchronous` | `SyncMode::Full` | durability level (`Full` / `Normal`) |

**`page_cache_size`** — File-backed databases only. The LRU cache holds recently read B+tree pages
in memory; a cache miss triggers a disk read. The default (256 pages = 1MB) covers a ~10K-fact
database comfortably. For a 100K-fact database with repeated queries over the same pages, raise to
1024–4096 pages. In-memory databases and the WASM browser backend have no use for this option — all
reads hit RAM regardless.

**`wal_checkpoint_threshold`** — Controls the write-latency / open-latency tradeoff. Lower = more
frequent checkpoints (smaller WAL, faster open, more checkpoint I/O). Higher = less frequent
checkpoints (larger WAL, faster writes, slower open on crash recovery). The default of 1000 suits
mixed workloads. For write-heavy batch ingestion, set to `usize::MAX` to disable auto-checkpoint
and call `db.checkpoint()` manually after the batch.

**`max_derived_facts` / `max_results`** — Safety limits, not performance knobs. Lower them to fail
fast on runaway recursive rules or unexpectedly large result sets; raise only if a legitimate query
hits the ceiling. For one-off queries that need a different limit without reconfiguring the database,
use `:max-derived-facts N` or `:max-results N` directly in the query vector — see the
[Datalog Reference](datalog-reference#max-derived-facts-and-max-results--per-query-complexity-limits).

**`synchronous`** — Controls WAL write durability, independent of `checkpoint()` (which always
fsyncs the main file regardless of this setting). `SyncMode::Full` (default) fsyncs after every
WAL write — every `transact`/`retract` is durable immediately, at the cost of a disk round-trip
per write. `SyncMode::Normal` skips that per-write fsync; writes are still safe across an ordinary
process crash, but only become disk-durable at the next checkpoint (auto-threshold, explicit, or
clean close), so a window of writes is lost on OS crash or power loss. Combine with batched writes
(see [Query Patterns](#batch-unrelated-writes-into-one-commit) below) for bulk loaders: open with
`Normal`, `wal_checkpoint_threshold` high or `usize::MAX`, write the whole batch, then call
`db.checkpoint()` once at the end.

---

## Query Patterns

### Anchor with a concrete entity or attribute

A full scan occurs only when both entity and attribute positions are unbound variables in every
pattern. Binding either triggers the selective index fetch path.

```datalog
; Full scan — both entity and attribute are variables
(query [:find ?e ?a ?v :where [?e ?a ?v]])

; Selective fetch — concrete attribute keyword
(query [:find ?e ?name :where [?e :person/name ?name]])

; Selective fetch — concrete entity keyword
(query [:find ?name :where [:alice :person/name ?name]])
```

Almost all real queries bind at least one attribute keyword, so selective fetch applies by default.

### Use `or` in a rule body, not mid-query

`or` mid-query re-evaluates branches over the full incoming binding set (O(N²)). The same logic
expressed as a rule starts from an empty binding and expands O(N).

```datalog
; O(N²) — or mid-query
(query [:find ?e :where (or [?e :tag :a] [?e :tag :b])])

; O(N) — equivalent rule body
(rule [(tagged ?e) [?e :tag :a]])
(rule [(tagged ?e) [?e :tag :b]])
(query [:find ?e :where (tagged ?e)])
```

### Keep `not` / `not-join` selective

The negation check scans all matching facts for each binding. Place the most selective positive
patterns before `not` so it sees a small binding set. The optimizer reorders positive patterns by
selectivity but does not move `not` / `or` — manual ordering still matters.

```datalog
; Worse — not sees all entities
(query [:find ?e
        :where [?e :role :admin]
               (not [?e :status :suspended])
               [?e :dept :engineering]])

; Better — most selective pattern first
(query [:find ?e
        :where [?e :dept :engineering]
               [?e :role :admin]
               (not [?e :status :suspended])])
```

### Batch unrelated writes into one commit

Each bare `db.execute()` call for a `transact`/`retract` triggers its own WAL write and, under the
default `SyncMode::Full`, its own fsync. `begin_write()` + N `execute()` calls + one `commit()`
already collapses all N facts into a single `tx_count` and a single WAL write — one fsync instead
of N — as long as the facts don't share the same entity + attribute + `:valid-from` + `:valid-to`
(that combination has its own semantics, unrelated to this optimization). Any call site issuing
several independent `db.execute()` calls in a row (e.g. writing a batch of unrelated entity
facts) should switch to this form.

```rust
let mut txn = db.begin_write()?;
txn.execute("(transact [[:e1 :name \"a\"]])")?;
txn.execute("(transact [[:e2 :name \"b\"]])")?;
txn.execute("(transact [[:e3 :name \"c\"]])")?;
txn.commit()?; // one WAL write, one fsync (at SyncMode::Full) for all three
```

### Use prepared queries for repeated patterns

`db.prepare()` pays the parse cost once. Each `execute()` substitutes bind values and runs the
already-parsed plan. Especially valuable for AI agents that issue the same query pattern in a loop.

```rust
let pq = db.prepare("(query [:find ?name :where [?e :person/name $name]])")?;
for name in names {
    let results = pq.execute(&[("name", BindValue::Val(Value::String(name)))])?;
}
```

### Limit recursive rule depth

Recursive rules use semi-naive fixed-point iteration. Each iteration extends the frontier by one
hop; deep chains require many iterations over growing intermediate tables. From benchmark data:
depth-10 chain = 2.75ms; depth-100 chain = 16s.

Where possible, bound the depth explicitly, or use `:max-derived-facts` as a backstop. Rule
invocations take one or two arguments, so a depth counter cannot be passed as a third argument.
Bound the depth by writing one rule per hop instead.

```datalog
; Unbounded — can be very slow on deep graphs
(rule [(reachable ?a ?b) [?a :edge ?b]])
(rule [(reachable ?a ?b) [?a :edge ?mid] (reachable ?mid ?b)])

; Bounded — at most 3 hops, one rule per depth
(rule [(hop-1 ?a ?b) [?a :edge ?b]])
(rule [(hop-2 ?a ?b) [?a :edge ?mid] (hop-1 ?mid ?b)])
(rule [(hop-3 ?a ?b) [?a :edge ?mid] (hop-2 ?mid ?b)])
(rule [(within-3 ?a ?b) (hop-1 ?a ?b)])
(rule [(within-3 ?a ?b) (hop-2 ?a ?b)])
(rule [(within-3 ?a ?b) (hop-3 ?a ?b)])

; Backstop for an unbounded rule: cap derived facts for this query only
(query [:find ?b
        :where (reachable :a ?b)
        :max-derived-facts 100000])
```

---

## Benchmark Reference

Full results with per-group commentary: [BENCHMARKS.md](https://github.com/project-minigraf/minigraf/blob/main/docs/BENCHMARKS.md).

Live history across releases: [bencher.dev/perf/minigraf/plots](https://bencher.dev/perf/minigraf/plots).

Key tables to check before tuning:
- **Query Latency** — cost at your target DB size
- **Database Open / Replay** — startup cost
- **Batch Insert Throughput** — ingestion planning
- **Negation** and **Disjunction** — O(N²) cost at scale

**Reproducing locally:**

```bash
# Run all Criterion benchmark groups (HTML report in target/criterion/)
cargo bench

# Run a specific group
cargo bench -- "insert"
cargo bench -- "query"
cargo bench -- "negation"
cargo bench -- "concurrent_btree_scan"

# Memory profile with heaptrack (requires heaptrack installed)
cargo build --release --example memory_profile
heaptrack ./target/release/examples/memory_profile 100000
heaptrack_print -f heaptrack.memory_profile.*.zst --merge-backtraces=0
```

---

## Platform Notes

| Platform | Differences |
|---|---|
| **Native (file-backed)** | Full feature set. WAL, checkpoint, `page_cache_size`, and `PreparedQuery` all apply. |
| **Native (in-memory)** | No WAL or checkpoint. `page_cache_size` has no effect — all reads hit RAM. |
| **WASM (browser)** | `BrowserBufferBackend` pre-loads all pages into RAM at open time. `wal_checkpoint_threshold` is ignored (no WAL sidecar). `page_cache_size` has no effect — same as in-memory native. |
| **WASI** | Same as native file-backed except filesystem access goes through the WASI sandbox. All knobs apply. |
| **Android / iOS (UniFFI)** | Same embedded model as native. Keep `wal_checkpoint_threshold` low (100–200) to reduce WAL replay time on cold open under OS storage restrictions. |
| **Python / Node.js / Java (FFI)** | `PreparedQuery` is not exposed over UniFFI or napi-rs — prepare/execute must be done in Rust. Per-call FFI overhead is negligible compared to query cost. |
