---
title: Upgrading to v3
nav: Upgrading to v3
section: Start
order: 5
since: v3.0.0
---
v3.0.0 is a major release: a new file format (v8) and fixes to data-integrity bugs that change query results for some existing data. This page lists what to check when you move from v2.x. The full list is in the [changelog](changelog#unreleased); [What changed](site:diff/?from=v2.0.4&to=v3.0.0) shows every documentation change between the two versions.

v2.x keeps getting data-integrity and security fixes for 12 months after v3.0.0 ships.

## Your files

- **A v7 file (written by v2.x) is upgraded to v8 the first time v3 opens it.** The upgrade re-indexes the file, is crash-safe, and is one way: v2.x cannot open a v8 file (`STG-006`). Keep a copy if you may need to go back; [Durability and Recovery](durability#upgrading) lists the steps. A file opened with `OpenOptions::read_only(true)` is read into memory instead and is not upgraded.
- **Formats v1–v6 are no longer readable** (`STG-028`). Open such a file once with v2.x to upgrade it to v7, then open it with v3.
- Files written by v3.0.0 development builds from before the final format fail with `STG-032`.
- In the browser, `BrowserDb.open()` writes the upgraded pages back to IndexedDB, so a stored v7 database is upgraded once rather than on every open.

## Query results that change

- **One current valid-time window per fact** (#435). A later `transact` of the same `(entity, attribute, value)` replaces its window; before, every window stayed current until a `retract`. A fact asserted in several transactions with different windows is now valid only in its latest window. If you relied on two windows of one fact being current at once, model each period as its own entity. See [One current valid-time window per fact](datalog-reference#one-current-valid-time-window-per-fact).
- **Same-transaction multi-values** (#371). Two values of one attribute written in one transaction are both returned by every query path. Before, only one came back.
- **Retract then re-assert in one write transaction** (#477). The last statement that writes a fact decides it at commit. Before, the retraction always won and the fact was gone after `commit()`.
- **Committed results come back in internal-id order** (entity, then attribute), not insertion or UUID order. Query results are sets; sort them if your code depends on an order.

## Errors that are new

| Was | Now |
|---|---|
| A `:find` variable no clause binds returned no rows | `PRS-080` at parse time |
| An unknown aggregate or window function returned no rows on empty data | `QRY-010` / `QRY-011` before the query runs |
| A valid-time window that ends at or before it starts was stored | `API-019`; nothing is written |
| One fact with two different windows in one `transact` | `API-011`; nothing is written |
| A whole serialised fact over 4,080 bytes | `WAL-003` for a string over 4,068 bytes or an attribute or keyword over 1,024 bytes |
| A query that reached a damaged index page fell back to a full scan | The page's `STG-029` / `STG-030` / `STG-031` |

Every code is described in the [error reference](error-reference).

## New APIs

- `Minigraf::query()` and `PreparedQuery::query()` return a [cursor](datalog-reference#cursors); `BrowserDb.query()` returns a `BrowserCursor`.
- `OpenOptions::read_only`, `Minigraf::fact_log()`, `LogWriter`, `Minigraf::verify()` and `Minigraf::rebuild_indexes()`: see [Reading, checking and copying databases](database-tools).

## Checkpoints

Checkpoints are copy-on-write and cost what changed rather than the size of the file: about 3 ms after one new fact at 10,000 or 100,000 facts. A crash at any point leaves the previous checkpoint intact, and opening never rebuilds an index. If you checkpointed rarely to avoid the cost, you can checkpoint more often. See [Performance tuning](performance-tuning).
