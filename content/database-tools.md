---
title: Reading, checking and copying databases
nav: Database tools
section: Reference
order: 25
since: v3.0.0
---
Rust APIs for tools that work on a whole database: backups, inspectors, integrity checks and offline migrations. The language bindings expose the same shape; see each binding's documentation for its names. For when to use them (backup and restore procedures, crash recovery, and what each storage error asks you to do), see [Durability and Recovery](durability).

## Read-only open

```rust
use minigraf::OpenOptions;

let db = OpenOptions::new().read_only(true).path("app.graph").open()?;
```

Nothing done through a read-only handle writes to the `.graph` file or its WAL.

- The file is locked in shared mode. Any number of read-only handles, in this process or others, can hold it at once. They exclude a read-write open, and it excludes them (`STG-025` / `STG-026`).
- The handle shows the same data as a read-write open: a WAL left by an earlier session is applied in memory but not checkpointed or deleted, and a format v7 file is read into memory instead of upgraded.
- A missing file is `STG-042`; nothing is created.
- `transact`, `retract`, `begin_write`, `checkpoint` and `rebuild_indexes` fail with `API-014`. Queries, cursors, `fact_log`, `verify` and rule registration work. Dropping the handle does not checkpoint.
- `page_cache_size` applies as usual, so a tool can keep the whole file resident.

## The fact log

`Minigraf::fact_log()` streams every fact version in the database, assertions and retractions, without Datalog:

```rust
use minigraf::{FactFilter, FactOrder};

let filter = FactFilter::new()
    .attribute_prefix(":order/")
    .tx_range(10..=20)
    .order(FactOrder::Tx);
for record in db.fact_log(&filter)? {
    let r = record?;
    // r.entity, r.attribute, r.value, r.tx_count, r.tx_id,
    // r.valid_from, r.valid_to, r.asserted
}
```

- `FactFilter` selects attributes (exact with `attributes`, or by `attribute_prefix`), `entities` and a `tx_range`. The filters are checked on index keys, so excluded records are never decoded.
- `FactOrder::Tx` (the default) returns records in ascending `tx_count`. The file has no index in transaction order, so the log holds at most `FactFilter::window` records (default 262,144) and scans the selected keys once per window. `FactOrder::Storage` reads in one pass, in storage order.
- The log reads the database as it was when the log opened. While a log is open, checkpoints are deferred: writes stay in the WAL, automatic checkpoints wait, and `checkpoint()` or `rebuild_indexes()` with work to do fail with `API-013`. Close, drop or finish the log to release it.
- Like a cursor, a `FactLog` is owned and `Send`. `next_batch(n)` returns records in batches.

## Building a database from records

`LogWriter` is the load step of an offline migration: it builds a new file from `FactRecord`s, keeping each record's `tx_count`, `tx_id`, valid-time window and assertion flag.

```rust
use minigraf::{FactFilter, LogWriter, OpenOptions};

let src = OpenOptions::new().read_only(true).path("old.graph").open()?;
let mut out = LogWriter::create("new.graph", OpenOptions::new())?;
for record in src.fact_log(&FactFilter::new())? {
    let r = record?;
    // Transform or drop records here.
    out.append(&r)?;
}
out.finish()?;
```

- Records must arrive in transaction order (`API-015`); gaps are allowed. Leaving a transaction out leaves a hole, and `:as-of` on it shows the state before. `advance_tx_count` keeps the source's counter after trailing purged transactions.
- One transaction has one `tx_id` (`API-016`), and values obey the usual size limits (`WAL-003`). A rejected record changes nothing.
- The file keeps the rules of a normal write: an empty or inverted window is `API-019`, a second window of one fact in one transaction `API-011`, and an assertion and a retraction of one fact in one transaction `API-020`. A source written before these rules (v2.x, or early v3 builds) can break them, and copying it fails at the first such record; repair those records in the loop. The [error reference](error-reference) describes each repair.
- There is no WAL. Records are committed in batches with the normal copy-on-write checkpoint, never splitting a transaction. The file is built at `<path>.partial` and renamed into place by `finish`, so a crash never leaves a partial file at `path`. An existing target or WAL is `STG-043`.

## Checking a file

```rust
let report = db.verify()?;
if !report.is_ok() {
    for problem in &report.problems {
        eprintln!("{problem}");
    }
}
```

Page checksums turn a torn or rotted page into an error on the read that reaches it (`STG-029` to `STG-031`). `verify()` finds the damage they cannot see: it walks every committed page and returns an `IntegrityReport` with one structured error per finding.

| Code | Finding |
|---|---|
| `STG-038` | An index holds different facts than EAVT, or a different count than the meta page |
| `STG-039` | Keys out of order, or a page reached twice |
| `STG-040` | Dictionary maps that disagree |
| `STG-036` | An id with no dictionary entry |
| `STG-035` | Leaked, doubly used or wrongly free pages |

It keeps no per-fact state: indexes are compared by an order-independent digest, so memory stays O(pages).

`rebuild_indexes()` rebuilds EAVT, AEVT, AVET and VAET from an intact index that agrees with another one (or the only intact one), derives the free list again, and commits like a checkpoint, including uncheckpointed writes. A crash at any point leaves the previous checkpoint. If no index can serve as the source, or the dictionary is damaged, it fails with `STG-041` and writes nothing.
