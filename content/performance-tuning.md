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
    .synchronous(SyncMode::Normal)  // skip per-write fsync; checkpoint stays durable
    .path("my.graph")
    .open()?;
```

| Option | Default | Unit |
|---|---|---|
| `page_cache_size` | 256 | pages (1 page = 4KB) |
| `wal_checkpoint_threshold` | 1000 | WAL entries |
| `max_derived_facts` | 1,000,000 | derived facts per rule iteration; from v3.0.0, in total |
| `max_results` | 1,000,000; from v3.0.0, `usize::MAX` | facts a rule query derives in total; a deprecated alias from v3.0.0 |
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
<!-- @until v2.0.5 -->
Before v2.0.5, closing a handle opened with `usize::MAX` still wrote its pending facts to the
`.graph` file (#476).
<!-- @end -->
<!-- @since v2.0.5 -->
`usize::MAX` also skips the checkpoint at close: the pending facts stay in the WAL, and the next
open replays them. Call `db.checkpoint()` before closing to write them into the `.graph` file.
<!-- @end -->

**`max_derived_facts` / `max_results`** — Safety limits, not performance knobs. Lower them to fail
fast on runaway recursive rules; raise only if a legitimate query hits the ceiling. Neither limits
the rows a query returns. For one-off queries that need a different limit without reconfiguring the
database, use `:max-derived-facts N` or `:max-results N` directly in the query vector — see the
[Datalog Reference](datalog-reference#max-derived-facts-and-max-results--per-query-complexity-limits).
<!-- @until v3.0.0 -->
`max_derived_facts` limits the facts derived in one rule iteration; `max_results` limits the facts
a rule query derives in total.
<!-- @end -->
<!-- @since v3.0.0 -->
From v3.0.0 there is one limit (#553): `max_derived_facts` limits the facts a rule query derives in
total, across iterations, and `max_results` is a deprecated alias of it: the lower of the two
applies, and `max_results` has no limit of its own by default, so it can lower the limit but not
raise it.
<!-- @end -->
<!-- @until v2.0.5 -->
Before v2.0.5, `max_results` also counts the database's own live facts, so with 1,000,000 or more
live facts every rule query fails with `INT-020` until the limit is raised (#528).
<!-- @end -->
<!-- @since v2.0.5 -->
The limit counts only the facts the rules derive, not the database's own facts.
<!-- @end -->

<!-- @since v3.0.0 -->
**`query_memory_limit`** — The most bytes one query may hold, default `usize::MAX` (no limit). A
query that would hold more fails with `QRY-012` and frees what it held; the database and other
queries carry on, and a cursor keeps the rows it already returned. What counts is the state
operators keep: a hash join's table, a negation's or an `or`'s table, a distinct set, an aggregate's
groups, a window's rows, the facts a rule query derives, and the query's entity and ident ids, each
by a per-entry estimate that is the same on every platform. Rows passing between operators are not
counted, and neither is the answer `execute` collects: read a large answer through `query`. A query
inside a write transaction that has staged writes is not counted. Set it on a device where one
query must not take the memory the app needs:

```rust
let db = OpenOptions::new()
    .query_memory_limit(64 * 1024 * 1024) // 64 MiB per query
    .path("my.graph")
    .open()?;
```
<!-- @end -->

<!-- @since v3.0.0 -->
**`allow_overrides`** — Which query keywords may override the database's settings (#869). Default
`Overridable::DEFAULTS`: `MaxDerivedFacts` (`:max-derived-facts`), `MaxResults`
(`:max-results`) and `MaxIterations` (`:max-iterations`, #861), so existing queries keep working. `QueryMemoryLimit` (`:query-memory-limit`,
#870) is not in the defaults. A query using a keyword the list leaves out is
refused with `API-025` before it runs, from `execute`, `query`, `prepare` and
`WriteTransaction::execute`; the override is never ignored or clamped. The list replaces the
defaults rather than adding to them, and it is not stored in the file. Set it when query text comes
from users or an LLM and must not raise your limits:

```rust
use minigraf::{OpenOptions, Overridable};

// No query keyword may override a limit.
let db = OpenOptions::new().allow_overrides([]).path("my.graph").open()?;

// The defaults plus another variant.
let opts = OpenOptions::new()
    .allow_overrides(Overridable::DEFAULTS.iter().copied().chain([Overridable::MaxResults]));
```

| Setting | Changed for one query by | Gated by `allow_overrides` |
|---|---|---|
| `max_derived_facts` | `:max-derived-facts N` or `$slot`; `PreparedQuery::with_max_derived_facts` | keyword yes (`MaxDerivedFacts`); setter no |
| `max_results` (deprecated) | `:max-results N` or `$slot` | yes (`MaxResults`) |
| `max_iterations` | `:max-iterations N` or `$slot`; `PreparedQuery::with_max_iterations` | keyword yes (`MaxIterations`); setter no |
| `query_memory_limit` | `:query-memory-limit N` or `$slot`; `PreparedQuery::with_query_memory_limit` | keyword yes (`QueryMemoryLimit`, not in `DEFAULTS`); setter no |
| other `OpenOptions` | open only | — |

`BrowserDb` takes no options and always uses the defaults.

**`max_iterations`** — The most rounds one stratum of a rule query may run (#861). Default
1,000,000, equal to `max_derived_facts`: a stratum starts another round only after deriving a new
fact, so at the defaults the derived-facts limit is reached first, and this one matters only when
lowered. Rounds are counted per stratum. A stratum still deriving after the limit fails with
`INT-020`. Before v3.0.0 the limit was a fixed 1,000 rounds.

```rust
let db = OpenOptions::new().max_iterations(10_000).path("my.graph").open()?;
```

**`PreparedQuery` limit setters** (#870, #861) — `with_max_derived_facts(n)`,
`with_max_iterations(n)` and `with_query_memory_limit(bytes)` replace the database values copied at `prepare` for that
statement. They are application code, so `allow_overrides` does not gate them; an allowed
keyword in the prepared text still overrides them. `usize::MAX` means no limit.

```rust
let pq = db
    .prepare("(query [:find ?x ?y :where (reach ?x ?y)])")?
    .with_max_derived_facts(10_000)
    .with_query_memory_limit(16 << 20);
```

`PreparedQuery` is `Clone` (#872): `pq.clone().with_max_iterations(10)` gives a second statement
with its own limits and leaves `pq` unchanged.

**In the language bindings** (#872) — Every binding takes the override list, reads the default
list from the core library (it is never written out again in a binding), takes
`transaction_chunk_facts` and `max_iterations` when it opens a database, and has the three setters
on its prepared query. Each setter returns a new prepared query and leaves the one it was called on
unchanged. As in Rust, a missing list means the defaults, a list replaces them, and an empty list
refuses every override keyword (`API-025`). A negative count is `API-017`. The no-limit value of a
setter is the one each binding uses for `query_memory_limit`.

| Binding | Override list | Default list | Open options | Setters | No limit |
|---|---|---|---|---|---|
| Python | `minigraf.open(..., allow_overrides=[Overridable.QUERY_MEMORY_LIMIT])` | `minigraf.OVERRIDABLE_DEFAULTS` | `transaction_chunk_facts=`, `max_iterations=` | `with_max_derived_facts`, `with_query_memory_limit`, `with_max_iterations` | `2**63 - 1` |
| Java | `Minigraf.options().allowOverrides(...)` | `Minigraf.OVERRIDABLE_DEFAULTS` | `transactionChunkFacts`, `maxIterations` | `withMaxDerivedFacts`, `withQueryMemoryLimit`, `withMaxIterations` | `Long.MAX_VALUE` |
| Android | `OpenOptions(allowOverrides = ...)` | `OVERRIDABLE_DEFAULTS` | `transactionChunkFacts`, `maxIterations` | `withMaxDerivedFacts`, `withQueryMemoryLimit`, `withMaxIterations` | `Long.MAX_VALUE` |
| Swift | `OpenOptions(allowOverrides: ...)` | `Overridable.defaults` | `transactionChunkFacts`, `maxIterations` | `withMaxDerivedFacts(n:)`, `withQueryMemoryLimit(bytes:)`, `withMaxIterations(n:)` | `Int64.max` |
| Node.js | `{ allowOverrides: ['maxDerivedFacts', 'maxResults', 'queryMemoryLimit', 'maxIterations'] }` | `OVERRIDABLE_DEFAULTS` | `transactionChunkFacts`, `maxIterations` | `withMaxDerivedFacts`, `withQueryMemoryLimit`, `withMaxIterations` | `Number.MAX_SAFE_INTEGER` or more |
| C | `"allow_overrides": ["max_derived_facts", "max_results", "query_memory_limit", "max_iterations"]` | `minigraf_overridable_defaults()` (JSON) | `"transaction_chunk_facts"`, `"max_iterations"` | `minigraf_prepared_with_max_derived_facts`, `…_with_query_memory_limit`, `…_with_max_iterations` | `SIZE_MAX` |
| Browser | none | none | none | `withMaxDerivedFacts`, `withQueryMemoryLimit`, `withMaxIterations` on `BrowserPreparedQuery` | 2^32 − 1 or more |

```python
db = minigraf.open("app.graph", allow_overrides=[], max_iterations=10_000)
reach = db.prepare("(query [:find ?x ?y :where (reach ?x ?y)])")
small = reach.with_max_derived_facts(10_000)  # reach keeps the database's limit
```

`BrowserDb` has only the setters: it takes no open options, so the default list applies and
`:query-memory-limit` in query text is refused. A browser setter takes a non-negative integer
number; a negative, fractional or non-finite one throws `[API-017]`.
<!-- @end -->

**`synchronous`** — Controls WAL write durability, independent of `checkpoint()` (which always
fsyncs the main file regardless of this setting). `SyncMode::Full` (default) fsyncs after every
WAL write — every `transact`/`retract` is durable immediately, at the cost of a disk round-trip
per write. `SyncMode::Normal` skips that per-write fsync; writes are still safe across an ordinary
process crash, but only become disk-durable at the next checkpoint (auto-threshold, explicit, or
clean close), so a window of writes is lost on OS crash or power loss. Combine with batched writes
(see [Query Patterns](#batch-unrelated-writes-into-one-commit) below) for bulk loaders: open with
`Normal`, `wal_checkpoint_threshold` high or `usize::MAX`, write the whole batch, then call
`db.checkpoint()` once at the end.

<!-- @since v3.0.0 -->
**Per-transaction options (`TxOptions`).** One `WriteTransaction` can choose its own sync mode
and chunk size with `begin_write_with`, without reopening the database; `begin_write()` uses the
database's options. A `None` field means the database value, and the options apply to that
transaction only:

```rust
use minigraf::{SyncMode, TxOptions};

// A database opened with the default SyncMode::Full.
let mut tx = db.begin_write_with(
    TxOptions::new()
        .synchronous(SyncMode::Normal) // no WAL sync at this commit
        .chunk_facts(16_384),          // this transaction's chunk size
)?;
tx.execute("(transact [[:e1 :name \"a\"]])")?;
tx.commit()?;
```

- `synchronous` decides only whether that transaction's WAL entry is synced. A later `Full`
  commit syncs the whole WAL, so it also makes the earlier `Normal` commits durable: a bulk load
  can run as `Normal` transactions on a `Full` database and end with one `Full` transaction (or
  `checkpoint()`). After a power loss the database holds a prefix of the commit order that
  includes every transaction up to the last `Full` commit or checkpoint.
- It has no effect on a transaction that becomes large (it commits as a checkpoint, durable in
  both modes) or on an in-memory database.
- `chunk_facts` replaces `transaction_chunk_facts` for that transaction, kept between 1 and
  1,000,000.
<!-- @end -->

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

<!-- @since v3.0.0 -->
The [manual v7-to-v8 migration sweep](https://github.com/project-minigraf/minigraf/blob/3b30398f408dd6d1c00e007126dc8e913fc61dbd/docs/benchmarks/838-migration/README.md)
records frozen fixtures, raw timings and correctness checks for native migration
through 200,000 facts and Chromium, Firefox and WebKit through 20,000 facts.
It separates nonmigrating native read-only open, migration and same-context reopen,
and checks bounded cursor snapshots during eight post-migration commits.
The 200,000-fact browser rung, cold startup and sustained pin/memory scaling remain
excluded. These manual measurements do not expand nightly benchmark coverage.
<!-- @end -->

<!-- @since v3.0.0 -->
The [manual shape sweep](https://github.com/project-minigraf/minigraf/blob/44e2d91c11568cc0d13189ad62cd6f4f2777ebd9/docs/benchmarks/841-shape-sweeps/README.md)
separates sorting, partition layouts, set cardinalities, reference distributions,
and sustained snapshot/log-reader retention. Read its pinned revision, storage
state and measurement limits alongside each result: a process RSS peak includes
more than the query meter charges, and a small output batch does not bound all
operator state. This manual evidence is separate from nightly coverage and
finite-memory release acceptance.

Three patterns from it matter when tuning. A cursor or `FactLog` held across
checkpoints keeps every page those checkpoints free, so the file grows until the
oldest reader is released; many readers cost what the oldest one does, and the
file does not shrink afterwards, though later checkpoints reuse the pages.
A window's retained state follows its largest partition, and a query whose
windows need incompatible partition or order keys holds the whole input. A descending (or
otherwise unsatisfied) `:order-by` under `:limit` keeps `offset + limit` rows,
so a deep offset retains about as many rows as it skips.
<!-- @end -->

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
