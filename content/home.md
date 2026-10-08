---
title: Minigraf documentation
nav: Overview
section: Start
order: 0
---
Minigraf is a tiny, portable bi-temporal graph database with Datalog queries: embedded, single-file, with time travel. These docs cover every release from v2.0.0 on. Pick a version at the top of the page; [What changed](site:diff/) compares two versions.

## Tutorials

A step-by-step introduction to Minigraf's Datalog dialect, driven by a real-world
e-commerce storyline. Each section builds on the last.

- [Setup — Install Minigraf and load the Corestore dataset](tutorial-setup)
- [1. Basic transact + query](tutorial-01-basic-transact-query)
- [2. `:as-of` — time travel by transaction time](tutorial-02-as-of)
- [3. `:valid-at` and `:any-valid-time` — time travel by valid time](tutorial-03-valid-at)
- [4. Recursive rules](tutorial-04-recursive-rules)
- [5. Negation — `not` and `not-join`](tutorial-05-negation)
- [6. Aggregates, `:with`, and window functions](tutorial-06-aggregates)
- [7. Expression clauses and predicates](tutorial-07-expressions)
- [8. Prepared queries with bind slots](tutorial-08-prepared-queries)
- [9. Disjunction — `or` and `or-join`](tutorial-09-disjunction)
- [10. User-defined functions](tutorial-10-udfs)
- [11. Marketplace — multi-seller queries](tutorial-11-marketplace)

## Cookbook

Problem-oriented recipes for common Minigraf patterns. Each recipe is self-contained:
a one-line problem statement, the Datalog (or Rust API where needed), and brief notes.

- [Graph Traversal Patterns](cookbook-graph-traversal) — neighbors, transitive closure,
  reachability, leaf detection, degree, property graphs, edge reification
- [Audit and Time-Travel Idioms](cookbook-time-travel) — point-in-time, time-interval,
  time-point lookup, and time-interval lookup across both time axes
- [Bitemporal Modeling](cookbook-bitemporal-modeling) — bounded facts, retroactive
  corrections, future-dating, overlapping periods, lifecycle end
- [Application Workflow Patterns](cookbook-application-workflows) — agent memory,
  offline-first state, task DAGs, GraphRAG, multi-tenant fleet patterns

## Pages

- **[Datalog Reference](datalog-reference)** — Complete syntax reference: facts, queries, recursive rules, bi-temporal operators, value types, and the EDN command syntax
- **[Time travel visualizer](https://project-minigraf.github.io/minigraf-visualizer/)** — Browser app that draws a database's history over transaction time and valid time. Cookbook recipes and tutorial sections link to it with their data already loaded.
<!-- @until v3.0.0 -->
- **[Architecture](architecture)** — Module structure, EAV data model, file format (v7), WAL layout, storage internals, and cross-platform workspace crates
<!-- @end -->
<!-- @since v3.0.0 -->
- **[Architecture](architecture)** — Module structure, EAV data model, file format (v8), WAL layout, storage internals, and cross-platform workspace crates
<!-- @end -->
- **[Use Cases](use-cases)** — Detailed guides for the three primary deployment targets: AI agents, mobile apps, and WASM/browser
- **[Comparison](comparison)** — Side-by-side comparison with XTDB, Cozo, Datomic, GraphLite, petgraph, IndraDB, SurrealDB, and time-series databases; temporal vs. time-series explainer
- **[Learning Resources](learning-resources)** — Curated links for Datalog, temporal databases, and SQLite internals
- **[Performance Tuning](performance-tuning)** — cost model, configuration knobs, query patterns, and benchmark reference

## Packages

| Platform | Package |
|---|---|
| Rust | [crates.io/crates/minigraf](https://crates.io/crates/minigraf) |
| Browser WASM | [@minigraf/browser on npm](https://www.npmjs.com/package/@minigraf/browser) |
| WASI (Node.js) | [@minigraf/wasi on npm](https://www.npmjs.com/package/@minigraf/wasi) |
| Node.js | [minigraf on npm](https://www.npmjs.com/package/minigraf) |
| Python | [minigraf on PyPI](https://pypi.org/project/minigraf) |
| Java/JVM | [minigraf-jvm on Maven Central](https://central.sonatype.com/artifact/io.github.project-minigraf/minigraf-jvm) |
| Android | [minigraf-android on Maven Central](https://central.sonatype.com/artifact/io.github.project-minigraf/minigraf-android) |
| iOS / macOS | Swift Package Manager: [minigraf-swift](https://github.com/project-minigraf/minigraf-swift) |
| C | [minigraf-c releases](https://github.com/project-minigraf/minigraf-c/releases) |

## Quick links

- [Repository](https://github.com/project-minigraf/minigraf)
- [README](https://github.com/project-minigraf/minigraf/blob/main/README.md)
- [ROADMAP.md](https://github.com/project-minigraf/minigraf/blob/main/ROADMAP.md)
- [BENCHMARKS.md](https://github.com/project-minigraf/minigraf/blob/main/docs/BENCHMARKS.md)
- [PHILOSOPHY.md](https://github.com/project-minigraf/minigraf/blob/main/PHILOSOPHY.md)
- [CONTRIBUTING.md](https://github.com/project-minigraf/minigraf/blob/main/CONTRIBUTING.md)
- **[Time travel visualizer](https://project-minigraf.github.io/minigraf-visualizer/)** — See Minigraf's bi-temporal history in your browser
- **[Demo: Temporal Reasoning](https://github.com/adityamukho/temporal_reasoning)** — Working AI agent using Minigraf's bi-temporal model
