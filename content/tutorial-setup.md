---
title: Corestore Tutorial — Setup
nav: Setup
section: Tutorial
order: 0
---
This tutorial series walks through Minigraf's Datalog query language from the ground up — basic facts and queries, bi-temporal operators (`:as-of`, `:valid-at`), recursive rules, negation, aggregates, expression clauses, prepared queries, disjunction and user-defined functions, in the backdrop of a fictional, multi-seller online marketplace. Every section builds on a shared dataset drawn from the Corestore storyline, so you see each feature in context rather than in isolation.

## Meet Corestore

Corestore is a fictional single-seller (initially) consumer electronics retailer. It stocks the usual suspects — laptops, phones, accessories — and processes a steady stream of orders from a small cast of customers. For most of the tutorial, there is one seller and one storefront. The final section, [Marketplace](tutorial-11-marketplace), introduces competing sellers to motivate more complex queries necessitated by the additional multi-seller layer.

## The cast

| Character | Archetype | Role in the tutorial |
|---|---|---|
| Alice | Frequent buyer | Establishes the basics — her orders are clean and predictable |
| Ben | Price-watcher | Anchors the bi-temporal sections; scrutinises price drops and receipt corrections |
| Clara | Complicated history | Drives negation, aggregates, and disjunction — split shipments, rescheduled deliveries, cancelled items |

## Prerequisites

You need a working Rust toolchain. If you do not have one:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then either install the published crate:

```bash
cargo install minigraf
```

Or clone and build from source:

```bash
git clone https://github.com/project-minigraf/minigraf
cd minigraf
cargo build
```

## Load the Corestore dataset

All tutorial sections share a common starting dataset. Load it once and persist it to a local file before working through any section:

```bash
cargo run --bin minigraf -- --file corestore.graph < demos/tutorial_corestore_setup.txt
```

This creates `corestore.graph` in the current directory and runs 3 transactions (tx_count 1–3): product catalogue, customer records, and initial orders. Every tutorial page assumes these facts are present and references them by name.

## Start the REPL

```bash
cargo run --bin minigraf -- --file corestore.graph
```

This opens `corestore.graph` (created above) and drops you into an interactive session. Type Datalog commands at the prompt. To exit, press `Ctrl-D` or type `EXIT`.

## A note on transaction counts

`tx_count` increments once per `transact` or `retract` call — never on queries. The tutorial sections must be run in order for `:as-of N` examples to produce the results shown. If you run extra transacts between sections (for example, to experiment), your tx_count will be higher than expected and the time-travel examples will not match. If that happens, the simplest fix is to delete `corestore.graph` and re-run the load command above.

**Visualize:** you can also explore this dataset without installing anything. The [time travel visualizer](https://project-minigraf.github.io/minigraf-visualizer/#sample=corestore-tutorial&tx=3) has it as a sample, with every write from sections 1 to 3 already applied and the same transaction numbers as the text. It opens at tx 3, right after setup.

## Tutorial sections

| Section | Topic |
|---|---|
| [Setup](tutorial-setup) | This page |
| [1. Basic transact + query](tutorial-01-basic-transact-query) | Alice places her first order |
| [2. `:as-of`](tutorial-02-as-of) | Ben's price drop — time travel by transaction time |
| [3. `:valid-at` and `:any-valid-time`](tutorial-03-valid-at) | Pricing correction — time travel by valid time |
| [4. Recursive rules](tutorial-04-recursive-rules) | Category hierarchy traversal |
| [5. Negation](tutorial-05-negation) | Clara's split shipment — `not` and `not-join` |
| [6. Aggregates and window functions](tutorial-06-aggregates) | Totals, rankings, and the `:with` trap |
| [7. Expression clauses](tutorial-07-expressions) | Filters, arithmetic, and string predicates |
| [8. Prepared queries](tutorial-08-prepared-queries) | Parse once, execute many with `$bind-slots` |
| [9. Disjunction](tutorial-09-disjunction) | `or` and `or-join` |
| [10. User-defined functions](tutorial-10-udfs) | Custom predicates and aggregates |
| [11. Marketplace](tutorial-11-marketplace) | Multi-seller queries |

## Reference

Full syntax — value types, operators, rule syntax, temporal modifiers — is in the [Datalog Reference](datalog-reference).
