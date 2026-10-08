---
title: Architecture
nav: Architecture
section: Reference
order: 20
---
## Module Structure

```
src/
├── main.rs                     — binary entry point (interactive REPL)
├── lib.rs                      — public API exports
├── db.rs                       — public embedded API: Minigraf, OpenOptions, WriteTransaction; register_aggregate / register_predicate for UDFs; prepare(query_str) -> PreparedQuery
├── repl.rs                     — interactive Datalog REPL console
├── temporal.rs                 — UTC timestamp parsing (avoids chrono CVE GHSA-wcg3-cvx6-7396)
├── wal.rs                      — write-ahead log: WalWriter, WalReader, CRC32 entries
├── error.rs                    — structured error codes: MinigrafError, ErrorCategory, ErrorCode registry (PRS/QRY/STG/WAL/API/INT); matches docs/ERROR_REFERENCE.md
├── browser/                    — browser WASM backend (`browser` feature)
│   ├── buffer.rs               — BrowserBufferBackend: in-memory pages with dirty-page tracking
│   └── indexeddb.rs            — IndexedDB persistence for the browser backend
├── graph/
│   ├── types.rs                — Fact, Value, EntityId, Attribute, VALID_TIME_FOREVER
│   └── storage.rs              — FactStorage: in-memory EAV store with temporal query methods
├── query/datalog/
│   ├── parser.rs               — EDN/Datalog parser (incl. not / not-join / expr / window clauses with safety checks)
│   ├── executor.rs             — query executor with temporal filtering, not/not-join post-filters, eval_expr/is_truthy, apply_post_processing (aggregation + window)
│   ├── functions.rs            — FunctionRegistry: string-keyed aggregate/window/predicate registry; AggregateDesc, AggState, WindowOps; AggImpl discriminator (Builtin(WindowOps) vs Udf(UdfOps)); UdfOps (type-erased init/step/finalise closures via Box<dyn Any + Send>); PredicateDesc (Arc<dyn Fn(&Value) -> bool + Send + Sync>); register_aggregate / register_predicate / register_builtin_aggregate; all built-in aggregates registered at startup
│   ├── matcher.rs              — pattern matching engine with variable unification
│   ├── magic_sets.rs           — magic-sets rewriting for demand-driven recursive rules (not applied to rules with not/not-join)
│   ├── evaluator.rs            — RecursiveEvaluator + StratifiedEvaluator + evaluate_not_join + apply_expr_clauses_in_evaluator
│   ├── stratification.rs       — DependencyGraph, stratify(): negative edges + cycle detection
│   ├── rules.rs                — RuleRegistry: thread-safe rule management; stratify() on register
│   ├── optimizer.rs            — selectivity-based query plan optimizer (disabled under wasm); Expr clauses passed through unchanged
│   ├── prepared.rs             — PreparedQuery: parse-once/execute-many with named $slot bind slots; BindValue enum (Entity/Val/TxCount/Timestamp/AnyValidTime); prepare_query(), substitute(); 19 unit tests
│   └── types.rs                — EdnValue (incl. BindSlot), Pattern, DatalogQuery, AsOf (incl. Slot), ValidAt (incl. Slot), WhereClause, BinOp, UnaryOp, Expr (incl. Slot), WindowFunc, Order, WindowSpec, FindSpec::Window; WindowFunc::Udf(String) and UnaryOp::Udf(String) variants for user-defined functions (resolved at runtime)
└── storage/
    ├── mod.rs                  — StorageBackend trait, FileHeader v7, CommittedFactReader / CommittedIndexReader traits
    ├── persistent_facts.rs     — PersistentFactStorage: v7 save/load, auto-migration v1–v6→v7, CommittedFactLoaderImpl
    ├── index.rs                — EAVT/AEVT/AVET/VAET key types, FactRef, encode_value
    ├── btree.rs                — legacy paged-blob B+tree (v5 migration only)
    ├── btree_v6.rs             — on-disk B+tree: build_btree, OnDiskIndexReader, MutexStorageBackend
    ├── cache.rs                — LRU page cache: approximate-LRU, read-lock on hits
    ├── packed_pages.rs         — packed fact page format (~25 facts/4KB), MAX_FACT_BYTES
    └── backend/
        ├── file.rs             — FileBackend: single .graph file, cross-platform
        ├── memory.rs           — MemoryBackend: in-memory backend for testing
        └── fault_inject.rs     — FaultInjectingBackend: injects I/O errors for durability tests (test builds only)
```

### Language bindings (separate repositories)

The bindings moved out of this repository in #231. Each one has its own repository and release pipeline, and is rebuilt when a new `minigraf` crate is published.

| Repository | Contents |
|---|---|
| [minigraf-python](https://github.com/project-minigraf/minigraf-python) | UniFFI bindings, `minigraf` on PyPI |
| [minigraf-java](https://github.com/project-minigraf/minigraf-java) | UniFFI bindings, `io.github.project-minigraf:minigraf-jvm` on Maven Central |
| [minigraf-android](https://github.com/project-minigraf/minigraf-android) | UniFFI bindings, `io.github.project-minigraf:minigraf-android` on Maven Central |
| [minigraf-swift](https://github.com/project-minigraf/minigraf-swift) | UniFFI bindings, `MinigrafKit` xcframework via Swift Package Manager |
| [minigraf-node](https://github.com/project-minigraf/minigraf-node) | napi-rs bindings, `minigraf` on npm |
| [minigraf-wasm](https://github.com/project-minigraf/minigraf-wasm) | `@minigraf/browser` and `@minigraf/wasi` on npm |
| [minigraf-c](https://github.com/project-minigraf/minigraf-c) | C FFI (cdylib + staticlib, `minigraf.h` via cbindgen) |

---

## Data Model

The unit of storage is a **Fact** — an Entity-Attribute-Value triple extended with bi-temporal metadata:

```rust
struct Fact {
    entity:    EntityId,   // Uuid
    attribute: Attribute,  // String, e.g. ":person/name"
    value:     Value,
    tx_id:     TxId,       // Uuid — transaction that asserted this fact
    tx_count:  u64,        // monotonic transaction counter (used for :as-of queries)
    valid_from: i64,       // Unix ms — when fact became valid in the real world
    valid_to:   i64,       // Unix ms — i64::MAX = open-ended (valid forever)
    asserted:  bool,       // true = assert, false = retract
}

enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Ref(Uuid),        // reference to another entity
    Keyword(String),  // e.g. ":status/active"
    Null,
}
```

`VALID_TIME_FOREVER = i64::MAX` is the sentinel for open-ended valid time.

---

## Storage Architecture

### Layered design

```
┌─────────────────────────────────────┐
│  Minigraf / WriteTransaction (db.rs)│  ← public API
├─────────────────────────────────────┤
│  FactStorage (graph/storage.rs)     │  ← in-memory EAV + index-driven scans
│  PersistentFactStorage              │  ← persistence layer
├─────────────────────────────────────┤
│  PageCache (storage/cache.rs)       │  ← LRU page cache (default 256 pages = 1MB)
├─────────────────────────────────────┤
│  StorageBackend trait               │  ← platform-agnostic page I/O
│  FileBackend / MemoryBackend        │
└─────────────────────────────────────┘
       ↕ WAL sidecar (wal.rs)
```

Pending (uncommitted) facts live in memory. Committed facts are stored in packed pages on disk and resolved on demand via the `CommittedFactReader` trait — no load-all at startup. Index lookups go through `OnDiskIndexReader` (Phase 6.5), which traverses B+tree pages via the LRU cache; index memory usage is O(cache_pages), not O(facts).

### Covering indexes

Four Datomic-style covering indexes are maintained for each committed fact:

| Index | Sort order | Best for |
|---|---|---|
| EAVT | entity → attribute → value → tx | entity lookups |
| AEVT | attribute → entity → value → tx | attribute scans |
| AVET | attribute → value → entity → tx | value equality lookups |
| VAET | value → attribute → entity → tx | reverse ref lookups |

Each index entry is a `FactRef { page_id, slot_index }` — a pointer to the fact's location in the packed pages. Values are encoded with sort-order-preserving byte representation so range scans work correctly.

#### Index keys in v3.0.0

> **Goes live in v3.0.0.** v2.x keeps the v7 layout above. In v7, `EavtKey` and `AevtKey` carry no value, so two values of one attribute written in the same transaction share a key, and reads can return only one of them (#371, #287).

From v3.0.0 every index entry *is* the whole fact, as a byte-comparable key, and `FactRef` is gone. See [File format v8](#file-format-v8--goes-live-in-v300).

---|---|---|
| EAVT | entity, attribute, valid_from, valid_to, tx_count | … + **value_bytes, asserted** |
| AEVT | attribute, entity, valid_from, valid_to, tx_count | … + **value_bytes, asserted** |
| AVET | attribute, value_bytes, valid_from, valid_to, entity, tx_count | … + **asserted** |
| VAET | ref_target, attribute, valid_from, valid_to, source_entity, tx_count | … + **asserted** |

- The new fields are **appended**, so entity and attribute range scans keep the same order. `value_bytes` is `encode_value(&value)`.
- `asserted` keeps an assertion and a retraction of the same value in the same transaction apart.
- Keys are built only through per-key constructors (`EavtKey::from_fact`, `EavtKey::entity_start`, `AevtKey::attribute_start`, …). Lookups scan from a start key and stop at the first non-matching key.
- The query-time deduplication in `selective_fact_fetch` also uses the full fact identity `(entity, attribute, tx_count, asserted, value, valid_from, valid_to)`.

---

## File Format (v7)

The `.graph` file is page-based (4KB pages), endian-safe, cross-platform.

```
Page 0: FileHeader (84 bytes)
  bytes  0.. 4   magic "MGRF"
  bytes  4.. 8   version u32 LE (currently 7)
  bytes  8..16   page_count u64 LE
  bytes 16..24   node_count u64 LE  (fact count)
  bytes 24..32   last_checkpointed_tx_count u64 LE
  bytes 32..40   eavt_root_page u64 LE  (B+tree root for each covering index)
  bytes 40..48   aevt_root_page u64 LE
  bytes 48..56   avet_root_page u64 LE
  bytes 56..64   vaet_root_page u64 LE
  bytes 64..68   index_checksum u32 LE  (CRC32 of committed fact pages)
  byte  68       fact_page_format u8    (0x02 = packed)
  bytes 69..72   _padding [u8; 3]
  bytes 72..80   fact_page_count u64 LE  (new in v6, retained in v7)
  bytes 80..84   header_checksum u32 LE  (new in v7 — CRC32 of header bytes 0..80)

Pages 1+: Packed fact data pages (page_type = 0x02)
  12-byte page header:
    byte  0       page_type (0x02)
    byte  1       _reserved (0x00)
    bytes 2..4    record_count u16 LE
    bytes 4..12   next_page u64 LE  (0 = no overflow)
  Record directory: record_count × 4 bytes
    per entry: offset u16 LE | length u16 LE
  Record data: variable-length postcard-serialised Facts
    (written end-to-start within the page)

Index pages (after fact pages): proper on-disk B+tree nodes (btree_v6.rs)
  Internal node page: sorted keys + child page IDs
  Leaf node page: sorted (key, FactRef) pairs + next_leaf pointer
  Each B+tree node is exactly one 4KB page
```

**Serialisation**: facts use [postcard](https://github.com/jamesmunns/postcard) — lightweight, embedded-focused, endian-safe.

**Migration**: `from_bytes` auto-migrates v1/v2/v3/v4/v5/v6 headers on open. v6 databases migrate to v7 on first checkpoint (header_checksum field added).

### File format v8 — goes live in v3.0.0

> **Not in any v2.x release.** Built on the `v3` branch for v3.0.0 (#374, #434, #388, #433, design: `docs/superpowers/specs/2026-10-05-v8-storage-format-design.md`). Until v3.0.0 ships, the current format is v7 as described above.

**Layout.**

```
Page 0, 1   Meta pages A and B. Odd generations commit to page 0, even to page 1.
            magic "MGRF"/"META", version 8, CRC32 over the whole page, generation,
            page_count, fact_count, last_checkpointed_tx_count, five tree roots
            (EAVT, AEVT, AVET, VAET, DICT), free-list head and count, next_eid,
            next_iid, required_features. The only commit point.
Page 2+     Any mix of B+tree leaves (0x61) and internal nodes (0x62), value
            pages (0x51) and free-list pages (0x81). Each starts with a 24-byte
            header: type, count, CRC32, its own page id, the generation that
            wrote it. Every read checks all four.
Sidecar     <db>.wal, version 2: the header records the base generation.
```

**Covering keys.** Every index entry is a whole fact, encoded so that comparing bytes gives the logical order:

| Tree | Key |
|---|---|
| EAVT | `e a v tx↓ vf vt op` |
| AEVT | `a e v tx↓ vf vt op` |
| AVET | `a v e tx↓ vf vt op` |
| VAET | `v a e tx↓ vf vt op` (ref values only) |
| DICT | UUID ↔ entity id, ident ↔ ident id, `tx_count` → `tx_id`, long-value dedup |

- Entities and idents (attribute names and keyword values) are sequential ids from the DICT tree, assigned at checkpoint. The query layer still sees UUIDs and strings.
- Integers use the FoundationDB tuple encoding; `tx↓` is the complemented `tx_count`, so the newest transaction comes first; `vt = FOREVER` is one byte.
- Strings over 64 bytes are stored once (deduplicated) in value pages; the key holds a 32-byte prefix, a hash and a reference.
- Leaves are prefix-compressed with restart points; internal nodes hold shortest separators. There are no leaf sibling pointers: scans use a cursor with a parent stack and a forward `seek`.
- A query reads only index leaves, the DICT pages that translate its ids, and a value page for a long string. Committed results come back in id order.

**Checkpoints are copy-on-write.**
- A checkpoint writes only:
  - new value pages;
  - the touched leaves and their paths to the root, in each of the five trees;
  - a few free-list pages;
  - the other meta page.
- Pages freed by one checkpoint are reused by later ones. The free list is read only as far as it is needed, and new pages are pushed onto its head.
- A crash or torn write at any point leaves the previous checkpoint intact. Open never rebuilds an index.
- Cost follows the change:

| Checkpoint | 10k facts | 100k facts | 1M facts |
|---|---|---|---|
| after 1 new fact | 3.0 ms | 3.3 ms | 6.1 ms |

  One new fact at 100k facts writes 16 pages.

**Density.** About 142 bytes per fact, history included, at 1.15M facts in #433's shape (about 10 attributes per entity, 20 % multi-valued, 10 % retracted and re-asserted). v7 used about 2 KB per fact as measured.

**Integrity.**
- A damaged page fails the read or checkpoint that reaches it, with `STG-029` (CRC), `STG-030` (page id) or `STG-031` (generation). It never returns wrong or fewer rows.
- If the newest meta page is damaged after its WAL is gone, open fails with `STG-033` rather than silently opening an older generation.
- Unknown `required_features` bits fail with `STG-034`.

**Limits.** String values up to 4,068 bytes; attribute names and keyword values up to 1,024 bytes (`WAL-003`).

**Migration.**
- v7 files (v2.x) upgrade automatically, one way, on first open.
- The upgrade is crash-safe, with a backup meta page. Old pages become free pages that later checkpoints reuse.
- v1–v6 fail with `STG-028`: open them once with v2.x first.
- Development builds of v3.0.0 from before this format fail with `STG-032`.

**Modules (v3.0.0, `src/storage/`):**
- `keys.rs`, `node.rs`, `btree.rs`: keys, nodes, the tree with `cow_insert` and `LeafCursor`.
- `dict.rs`, `value_pages.rs`, `reader.rs`: dictionary and encoder, long values, covering reads.
- `meta.rs`, `page.rs`, `freelist.rs`: meta pages, page header and allocator, free list.
- `packed_pages.rs`: v7 reader for migration.

---

## WAL (Write-Ahead Log)

The WAL sidecar (`<db>.wal`) is present whenever there are uncommitted writes. It is replayed on open and deleted on checkpoint.

```
WAL file layout:
  Header: magic "MWAL", version u32
  Entries (repeated):
    checksum u32     — CRC32 of the rest of the entry
    tx_count u64     — transaction counter
    num_facts u64    — number of facts in this entry
    [ len u32 | postcard-bytes ]×num_facts
```

CRC32-protected entries ensure partial writes (from crashes) are safely discarded. Every WAL write is followed by a flush to disk, controlled by `OpenOptions::synchronous` (see [Performance Tuning](performance-tuning#configuration-knobs)):

- **`SyncMode::Full`** (default) — `fdatasync` after every entry. Matches Minigraf's original always-fsync behavior; every committed `transact`/`retract` is durable immediately.
- **`SyncMode::Normal`** — no per-write flush. Entries are still `write_all()`'d (safe across an ordinary process crash) but not forced to disk until the next checkpoint (auto-threshold, explicit `checkpoint()`, or clean close). Data written since the last checkpoint is lost only on OS crash or power loss, not process death. Intended for bulk loaders that can safely re-run from a checkpoint watermark.

`checkpoint()` itself always fsyncs the main `.graph` file regardless of `synchronous` — it remains the hard durability boundary in both modes.

---

## Query Execution Pipeline

1. **Parse** — EDN string → `DatalogQuery`; `not` / `not-join` / `Expr` clauses safety-checked at this stage; regex patterns in `matches?` validated at parse time
2. **Plan** — `optimizer.rs` selects an index hint and reorders join clauses by selectivity; `Expr` clauses are passed through unchanged (not reordered — ordering guaranteed by safety check)
3. **Execute** — `executor.rs` iterates patterns, resolves `FactRef`s via page cache, applies temporal filter:
   - Step 1: tx-time filter (`:as-of` counter or timestamp)
   - Step 2: net-assertion filter — per `(entity, attribute, value)` triple, keep only the latest `tx_count`; discard if that record is a retraction (`asserted = false`)
   - Step 3: valid-time filter (`:valid-at` or `:any-valid-time`)
   - Step 4: not / not-join post-filter — applied per candidate binding after pattern matching
   - Step 5: `Expr` clause evaluation (`apply_expr_clauses`) — filter predicates drop non-truthy rows; arithmetic bindings extend the binding with the result value; type mismatches and div/0 silently drop the row
   - Step 6: `apply_post_processing` — aggregates collapse rows (grouping by plain-variable `:find` specs via `FunctionRegistry`); window functions annotate per-row (sort within partition by `:order-by` key, walk accumulating window state, emit one output row per input row)
4. **Evaluate rules** — `StratifiedEvaluator` stratifies rules; for each stratum:
   - Positive rules evaluated via `RecursiveEvaluator` (semi-naive fixed-point iteration)
   - Mixed rules (containing `not` / `not-join` / `Expr`) run positive patterns first, then apply negation filters and expr clauses per binding

### Rule Registration

`register_rule` calls `stratify()` after adding each rule. If the new rule creates a negative cycle in the dependency graph, `stratify()` returns `Err` and the rule is not registered. Non-recursive negation is always safe.

---

## File Locking

A `.graph` file is guarded by a kernel file lock taken on the file itself
(`std::fs::File::try_lock`: `flock` on Unix, `LockFileEx` on Windows). The lock
is released by the kernel whenever the holding process exits, however it exits,
so a crashed holder never leaves the database unopenable. Because `flock` and
OFD locks attach to the open file description rather than the process, a second
open within one process is refused too.

There is no lock file. A `.graph.lock` sidecar left behind by versions before
2.0 is ignored and never deleted, since a still-running old process may depend
on it. Running mixed versions against one file is not supported.

On a filesystem that cannot lock at all, `open()` fails rather than proceeding
unprotected. `OpenOptions::allow_unlocked(true)` overrides this, and accepts the
corruption risk that comes with it.

On Windows these locks are mandatory rather than advisory, and they exclude
every handle but the one holding them — including another handle in the same
process. While a database is open you cannot read its `.graph` file through a
second handle, your own included; the attempt fails with os error 33, "another
process has locked a portion of the file", even when that process is you.
Close the database first. On Unix the lock is advisory and such a read
succeeds.

A cross-process `WouldBlock` is retried with bounded backoff (10 attempts,
5ms doubling, capped at 50ms, ~375ms total) before being reported as a real
conflict, to ride out the transient window where a forked-but-not-yet-exec'd
subprocess holds a duplicate of someone else's lock — the same class of
problem SQLite's `busy_timeout` solves. A same-process conflict is not
retried; it fails immediately, since waiting could never help.

**One handle per file, per process.** A second `open` on a file this process
already has open is refused, naming the same-process case. This matters because
each `FileBackend` caches its own `header.page_count`, allocates new pages from
that count, and bounds-checks `read_page` against it — two handles on one file
give two page tables that diverge, which surfaces as
`Page N out of bounds (total pages: M)` and, past that, structural corruption.
`Minigraf` is cheap to clone and all clones share one database, so cloning the
existing handle is always the right move (#304).

---

## Thread Safety

- Concurrent reads via `Arc<RwLock<FactStorage>>`
- Exclusive write via `Mutex<WriteTransaction>`
- One live handle per file per process, enforced by the kernel file lock (see File Locking above)
- Rule registry is independently `Arc<RwLock<RuleRegistry>>`
- Function registry (`FunctionRegistry`) is `Arc<RwLock<FunctionRegistry>>` — shared between all query executions; built-in aggregates registered at startup; user-defined aggregates and predicates registered via `register_aggregate` / `register_predicate` (Phase 7.7b)
- Page cache uses read-lock on hits, write-lock only on misses — minimises contention for read-heavy workloads
- `BrowserDb` (`wasm32-unknown-unknown`) runs single-threaded — all `Arc`/`RwLock`/`Mutex` calls compile as single-threaded stubs under the `browser` feature; no WASM thread support
- `PreparedQuery` holds `Arc` clones of `FactStorage`, `RuleRegistry`, and `FunctionRegistry` — each `execute()` call re-reads live store state (new facts visible) while the query plan is reused
