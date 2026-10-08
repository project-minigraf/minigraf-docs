---
title: Comparison
nav: Comparison
section: Reference
order: 50
---
## Feature matrix

| Feature | Minigraf | XTDB | Cozo | Neo4j | SQLite |
|---|---|---|---|---|---|
| **Query Language** | Datalog | Datalog | Datalog | Cypher | SQL |
| **Single File** | ✅ Yes | ❌ No | ❌ No | ❌ No | ✅ Yes |
| **Bi-temporal** | ✅ Yes | ✅ Yes | ⚠️ Time travel | ❌ No | ❌ No |
| **Embedded** | ✅ Yes | ✅ Yes | ✅ Yes | ❌ No | ✅ Yes |
| **Graph Native** | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes | ❌ No |
| **Window Functions** | ✅ Yes | ✅ Yes | ✅ Yes | ⚠️ Limited | ✅ Yes |
| **User-Defined Functions** | ✅ UDF aggregates + predicates (v0.17.0) | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes |
| **Prepared Statements** | ✅ `$slot` temporal bind tokens (v0.18.0) | ⚠️ Limited | ❌ No | ✅ Yes | ✅ Yes |
| **Rust** | ✅ Yes | ❌ Clojure | ✅ Yes | ❌ Java | ❌ C |
| **WASM Ready** | ✅ Yes (browser, WASI) | ❌ No | ⚠️ Limited | ❌ No | ✅ Yes |
| **Platform support** | Tier 1: Rust, Python. Tier 2 (experimental): WASM, WASI, Android, iOS, Node.js, Java, C | JVM only | Native, WASM (limited) | JVM only | Native, WASM |
| **Maturity** | Young (v1.0 in 2026). Known data-integrity issues on v2.x, fixed in v3.0.0 | Mature | Pre-1.0 | Mature | Decades of production use |

Minigraf's bindings are split into [support tiers](https://github.com/project-minigraf/minigraf/blob/main/PHILOSOPHY.md#support-tiers). Before adopting v2.x, read the [known issues in the current release](https://github.com/project-minigraf/minigraf/issues/421).

---

## XTDB (formerly Crux)

- ✅ **Minigraf**: Single `.graph` file, simpler scope, Rust, WASM target
- ✅ **XTDB**: More mature, production-ready, battle-tested bi-temporal Datalog
- ❌ **XTDB**: Clojure + JVM, multi-file storage (directories), client-server in typical deployments

XTDB is the primary inspiration for Minigraf's temporal model. If you need production-grade bi-temporal Datalog today and a JVM is acceptable, XTDB is the better choice. Minigraf aims to be the single-file, embedded, Rust alternative.

**Try it:** [See bi-temporal Datalog in your browser — no install needed →](https://minigraf-playground.vercel.app/)

---

## Cozo

- ✅ **Minigraf**: Single file, bi-temporal first-class, WAL crash safety
- ✅ **Cozo**: More features (vector search, graph algorithms, time travel via historical mode), active development
- ❌ **Cozo**: Multi-file storage (RocksDB/Sled backends), no true bi-temporal model (historical mode is append-only, not full bi-temporal)

Cozo is the closest Rust competitor. The key differentiators: Minigraf has a proper bi-temporal model (independent transaction time and valid time); Cozo's "historical" mode is closer to append-only event sourcing. Minigraf is single-file; Cozo requires a directory.

**Try it:** [See bi-temporal Datalog in your browser — no install needed →](https://minigraf-playground.vercel.app/)

---

## Datomic

- ✅ **Minigraf**: Single file, embedded, open source, Rust
- ✅ **Datomic**: Production-proven since 2012, the canonical Datalog temporal database, excellent tooling
- ❌ **Datomic**: Client-server, Clojure + JVM, proprietary licence (free tier limited), multi-node storage

Datomic is the other major inspiration alongside XTDB. Minigraf borrows the EAV model, the four covering indexes (EAVT/AEVT/AVET/VAET), and the temporal query semantics. The positioning is complementary: Minigraf is for embedded use cases where Datomic cannot go.

**Try it:** [See bi-temporal Datalog in your browser — no install needed →](https://minigraf-playground.vercel.app/)

---

## GraphLite

- ✅ **Minigraf**: Datalog (recursive rules), bi-temporal, WAL crash safety
- ✅ **GraphLite**: Full GQL (ISO graph query language) spec compliance, more mature
- ❌ **GraphLite**: Multi-file storage (Sled directories), no bi-temporal support

If you need GQL compliance rather than Datalog, GraphLite is the better choice.

**Try it:** [See bi-temporal Datalog in your browser — no install needed →](https://minigraf-playground.vercel.app/)

---

## petgraph

- ✅ **Minigraf**: Persistent database with Datalog queries, bi-temporal time travel, ACID transactions
- ✅ **petgraph**: Dominant Rust graph *algorithms* library — BFS, DFS, Dijkstra, topological sort, strongly-connected components; fast, well-maintained
- ❌ **petgraph**: In-memory only, no persistence, no query language, no time travel

**These are not competing tools.** petgraph is the right choice when you need graph algorithms over an in-memory structure. Minigraf is the right choice when you need a persistent, queryable, time-aware graph store. They can be used together: load a subgraph from Minigraf into petgraph for algorithm execution, write results back.

**Try it:** [See bi-temporal Datalog in your browser — no install needed →](https://minigraf-playground.vercel.app/)

---

## IndraDB

- ✅ **Minigraf**: Single `.graph` file, bi-temporal first-class, Datalog queries, WAL crash recovery
- ✅ **IndraDB**: Rust embedded graph database with pluggable backends (in-memory, RocksDB), property graph model, more mature
- ❌ **IndraDB**: No bi-temporal support; RocksDB backend is multi-file; property graph model (not EAV/Datalog)

**Try it:** [See bi-temporal Datalog in your browser — no install needed →](https://minigraf-playground.vercel.app/)

---

## SurrealDB

- ✅ **Minigraf**: Embedded library, single file, zero configuration, bi-temporal, ~1.2MB binary budget
- ✅ **SurrealDB**: Multi-model database (graph, document, relational), distributed, mature, large ecosystem, SurrealQL
- ❌ **SurrealDB**: Client-server oriented, no single-file option, no bi-temporal model

**Not competing tools.** SurrealDB targets teams that need a full-featured distributed database with a rich query language. Minigraf targets developers who want to embed a lightweight bi-temporal graph store directly in their application — no server, no configuration, one file.

**Try it:** [See bi-temporal Datalog in your browser — no install needed →](https://minigraf-playground.vercel.app/)

---

## InfluxDB / Prometheus / TimescaleDB (time-series databases)

These are frequently confused with temporal databases. They are different categories.

| | **Temporal database** (Minigraf) | **Time-series database** (InfluxDB, Prometheus, TimescaleDB) |
|---|---|---|
| **Primary question** | "What was the state of entity X at time T?" | "What was the reading of metric M during window [T1, T2]?" |
| **Data unit** | Version-controlled entity history (like Git commits) | High-frequency measurement or event stream |
| **Time model** | Bi-temporal: valid time + transaction time, per entity | Append-only log, optimised for recency |
| **Query style** | Datalog time-travel across linked entity histories | Aggregation, downsampling, windowed statistics |
| **Strengths** | Referential integrity across history, audit trails, point-in-time snapshots | Fire-hose ingestion, alerting, dashboards, metric rollups |
| **Not suited for** | High-frequency sensor/metric ingestion | Entity history, time travel, graph traversal |

Use Minigraf when you need to ask *"what did my data look like at a point in the past?"* across linked entities — agent beliefs, audit records, knowledge graphs, correction histories. Use InfluxDB or Prometheus when you need to ingest thousands of IoT readings per second and visualise trends or set alerts.

**Try it:** [See bi-temporal Datalog in your browser — no install needed →](https://minigraf-playground.vercel.app/)

---

## DuckDB

- ✅ **Minigraf**: Bi-temporal model, Datalog with recursive rules, graph-native EAV, single `.graph` file, Rust
- ✅ **DuckDB**: Extremely fast analytical SQL (OLAP-optimised columnar storage), excellent ecosystem (Python/R/Node.js/Rust bindings), WASM build, production-ready (v1.0 released 2024)
- ❌ **DuckDB**: No graph model, no bi-temporal support, SQL only, C++ (not Rust)

DuckDB is an embedded, single-file OLAP database designed for analytical workloads — column scans, aggregations, window functions, and data-frame processing at high speed. Minigraf's strength is the opposite end of the spectrum: low-latency traversal and time-travel over connected, version-controlled entity graphs.

**Complementary, not competing.** A realistic combination: store agent beliefs or domain entities in Minigraf (temporal knowledge graph), periodically snapshot query results into DuckDB for aggregation and reporting. Each tool does what it does best.

**Try it:** [See bi-temporal Datalog in your browser — no install needed →](https://minigraf-playground.vercel.app/)

---

## Summary

**Minigraf's unique position**: single-file + embedded + bi-temporal + Datalog + Rust. No other database offers this exact combination. The closest in spirit is XTDB (bi-temporal Datalog) and SQLite (single-file embedded), and Minigraf is explicitly designed to occupy the intersection.
