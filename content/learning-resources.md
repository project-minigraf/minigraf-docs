---
title: Learning Resources
nav: Learning resources
section: Reference
order: 70
---
## Minigraf Datalog Tutorial

- [Tutorial Series](tutorial-setup) — a hands-on, eleven-section walkthrough of Minigraf's
  Datalog dialect, driven by an e-commerce storyline (Corestore). Covers every language
  feature from basic transact/query through recursive rules, bi-temporal queries, negation,
  aggregation, window functions, prepared queries, disjunction, UDFs, and marketplace
  multi-entity scenarios.

## Datalog — Further Reading

- [Learn Datalog Today](http://www.learndatalogtoday.org/) — interactive tutorial, browser-based exercises
- [Datomic Query Tutorial](https://docs.datomic.com/query/query-tutorial.html) — Datomic's Datalog dialect; most concepts transfer directly to Minigraf
- [XTDB Datalog Queries](https://xtdb.com/docs/) — XTDB's Datalog reference; the temporal query semantics closely match Minigraf's

## Temporal Databases

- [Exploring Temporality in Databases](https://adityamukho.com/exploring-temporality-in-databases) — conceptual deep-dive into transaction time, valid time, bi-temporal models, and beyond; includes an [interactive 3D visualisation](https://www.geogebra.org/m/ey3sky2s) of the bi-temporal state space
- [Temporal Query Types](https://recallgraph.hashnode.dev/temporal-query-types) — taxonomy of point-in-time, time-interval, time-point lookup, and time-interval lookup query classes; maps to Minigraf's `:as-of`, `:valid-at`, `:any-valid-time`, and (Phase 7) `:db/valid-from`/`:db/valid-to` pseudo-attributes
- [Temporal Database — Wikipedia](https://en.wikipedia.org/wiki/Temporal_database) — overview of transaction time, valid time, and bi-temporal concepts
- [XTDB Bitemporality](https://v1-docs.xtdb.com/concepts/bitemporality/) — XTDB's explanation of bi-temporal data; the model Minigraf is most directly inspired by
- [Datomic Time Model](https://docs.datomic.com/time/time-model.html) — Datomic's treatment of immutable history and time travel

## SQLite Internals (storage inspiration)

- [SQLite File Format](https://www.sqlite.org/fileformat.html) — page-based storage format that inspired Minigraf's `.graph` format
- [SQLite OS Interface (VFS)](https://www.sqlite.org/vfs.html) — SQLite's platform abstraction layer; analogous to Minigraf's `StorageBackend` trait
