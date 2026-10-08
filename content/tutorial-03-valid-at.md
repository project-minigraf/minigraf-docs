---
title: Tutorial 3: `:valid-at` and `:any-valid-time` — the second time axis
nav: 3. `:valid-at` / `:any-valid-time`
section: Tutorial
order: 3
---
← [Section 2: `:as-of`](tutorial-02-as-of) | → [Section 4: Recursive rules](tutorial-04-recursive-rules)

---

## The second time axis

Minigraf tracks two independent time axes. Transaction time — queried with `:as-of` — records *when the database learned something*. Valid time is different: it records *when a fact was true in the real world*, independent of when it was entered. You supply valid time yourself via `{:valid-from "..." :valid-to "..."}` on a `transact` call. A fact with `valid-from "2026-01-01"` and `valid-to "2026-02-28"` is asserted to have been true from January 1 through February 28, regardless of which transaction recorded it or when. `:valid-at "date"` filters the database to show only facts whose valid window covers that date, giving you a consistent view of the world as it was on that day.

---

## Scenario

Corestore runs a winter sale on the LaptopPro 15 from January 1 through February 28. A support agent accidentally enters the sale price as $1,099 when it should have been $1,049. The error is caught in May, after the sale has ended. The agent retracts the wrong entry and records the corrected price with the same valid window — the historical record is now accurate, even though the correction happened months later.

---

## Recording sale prices with valid time (tx 9 + tx 10)

**Cumulative state at the start of this section:** tx_count = 8
(tx 1: categories, tx 2: products, tx 3: customers, tx 4: Alice's order, tx 5: Alice's USB cable retract, tx 6: Ben's order, tx 7: retract old price, tx 8: new price $1,259)

```datalog
; tx 9: Winter sale price — valid Jan 1 through Feb 28
(transact {:valid-from "2026-01-01" :valid-to "2026-02-28"}
          [[:laptop-pro :product/sale-price 1099]])

; tx 10: Spring sale price — valid May 20 through Jun 30
(transact {:valid-from "2026-05-20" :valid-to "2026-06-30"}
          [[:laptop-pro :product/sale-price 1149]])
```

The `{:valid-from "..." :valid-to "..."}` map applies to every fact in the following vector. Both dates are inclusive ISO 8601 strings. Omitting `:valid-to` means the fact is valid forever — Minigraf stores this as a sentinel value (`VALID_TIME_FOREVER`) representing the far future.

After tx 10, the database has two sale-price facts for `:laptop-pro`: one covering the winter window and one covering the spring window.

---

## Correcting the error (tx 11 + tx 12)

```datalog
; tx 11: Retract the wrong winter sale price
(retract [[:laptop-pro :product/sale-price 1099]])

; tx 12: Record the correct winter sale price — same valid window
(transact {:valid-from "2026-01-01" :valid-to "2026-02-28"}
          [[:laptop-pro :product/sale-price 1049]])
```

The corrected fact has a new `tx_count` (12) but the same `valid-from`/`valid-to` as the erroneous entry. This is the canonical pattern for retroactively fixing a recording error: retract the wrong fact, then assert the right one with the original valid window. The transaction record is immutable — tx 9 and tx 11 remain in the history forever — but the *current* view of the world now reflects the correct $1,049 price.

---

## Query: what was the sale price on January 15?

```datalog
(query [:find ?price
        :valid-at "2026-01-15"
        :where [:laptop-pro :product/sale-price ?price]])
```

```
?price
--------------------
1049

1 result(s) found.
```

`:valid-at "2026-01-15"` filters to facts whose valid window contains January 15. After the correction, only the $1,049 fact covers that date — the $1,099 fact was retracted at tx 11. This reflects the database's *current* knowledge about what was true in January.

---

## Query: what sale prices are active right now?

Without any temporal filter, a query sees all net-asserted facts evaluated against the current date.

```datalog
(query [:find ?price
        :where [:laptop-pro :product/sale-price ?price]])
```

```
No results found.
```

Today is May 13, 2026. The winter sale ended February 28 and the spring sale starts May 20 — no sale-price fact has a valid window that covers today. The query returns an empty result set, which is the correct answer: there is no active sale price right now.

---

## The bi-temporal matrix: `:as-of` + `:valid-at` combined

Both time axes can be combined in a single query. This answers the most precise possible question: "what did the system *record* (as of tx N), about what was *true* (on that date)?"

```datalog
; What did the system record about Jan 15, as of tx 9 — before the correction?
(query [:find ?price
        :as-of 9
        :valid-at "2026-01-15"
        :where [:laptop-pro :product/sale-price ?price]])
```

```
?price
--------------------
1099

1 result(s) found.
```

At tx 9, the erroneous $1,099 fact exists and its valid window covers January 15. The retraction at tx 11 has not happened yet from this vantage point. This is what the system was telling customers before the mistake was caught.

```datalog
; What does the system now record about Jan 15 — after the correction at tx 12?
(query [:find ?price
        :as-of 12
        :valid-at "2026-01-15"
        :where [:laptop-pro :product/sale-price ?price]])
```

```
?price
--------------------
1049

1 result(s) found.
```

After the correction, the database records $1,049 as the price for January 15. The history of the error is preserved in transaction time (tx 9 and tx 11 still exist), but the corrected state is now visible from tx 12 onward.

### The bi-temporal matrix at a glance

| | Valid at Jan 15 | Valid at Jun 1 |
|---|---|---|
| As of tx 9 (before fix) | $1,099 | no sale |
| As of tx 12 (after fix) | $1,049 | $1,149 |

**Visualize:** open the matrix in the time travel visualizer: [as of tx 9 (before the fix)](https://project-minigraf.github.io/minigraf-visualizer/#sample=corestore-tutorial&tx=9&vt=2026-01-15&e=:laptop-pro&view=map) and [as of tx 12 (after the fix)](https://project-minigraf.github.io/minigraf-visualizer/#sample=corestore-tutorial&tx=12&vt=2026-01-15&e=:laptop-pro&view=map), both at January 15. Drag the crosshair on the bitemporal map to move through the other cells.

Each cell answers a distinct question. The upper-left cell is what the system believed about January 15 before the error was caught. The lower-left is the corrected knowledge. The lower-right shows that the spring sale ($1,149) was recorded in tx 10, which is visible from both tx 10 onward and covers June 1.

---

## `:any-valid-time` — the full audit trail

```datalog
(query [:find ?price
        :any-valid-time
        :where [:laptop-pro :product/sale-price ?price]])
```

```
?price
--------------------
1049
1149

2 result(s) found.
```

`:any-valid-time` lifts the valid-time filter entirely. Instead of asking "what is true on a specific date?", it asks "what are all the currently-asserted sale prices, regardless of when they are valid?" The result contains both the corrected winter price ($1,049) and the spring price ($1,149).

Note that the retracted $1,099 fact does **not** appear, even with `:any-valid-time`. This modifier lifts the *valid-time* filter, not the *retraction* filter. Retracted facts are always excluded from query results — they exist only in the transaction history, accessible via `:as-of` queries that predate the retraction.

---

## Per-fact valid time — the 4-element map form

The `{:valid-from :valid-to}` map at the transaction level applies the same valid window to every fact in the vector. When different facts in the same `transact` need different valid windows, attach a per-fact map as the fourth element of the tuple:

```datalog
; Equivalent to: (transact {:valid-from "2026-01-01" :valid-to "2026-02-28"}
;                           [[:laptop-pro :product/sale-price 1049]])
(transact [[:laptop-pro :product/sale-price 1049 {:valid-from "2026-01-01" :valid-to "2026-02-28"}]])
```

This form lets you mix valid windows within a single `transact`:

```datalog
(transact [
  [:laptop-pro :product/sale-price 1049 {:valid-from "2026-01-01" :valid-to "2026-02-28"}]
  [:laptop-pro :product/sale-price 1149 {:valid-from "2026-05-20" :valid-to "2026-06-30"}]
])
```

This is a matter of convenience, not semantics — both forms produce identical facts in the database.

---

## When to use which

| Goal | Use |
|---|---|
| "What did the system know at checkout?" | `:as-of N` |
| "What was actually true on that date?" | `:valid-at "date"` |
| "What did we know, about what was true?" | `:as-of N :valid-at "date"` |
| "Show me every recorded version" | `:any-valid-time` |

---

← [Section 2: `:as-of`](tutorial-02-as-of) | → [Section 4: Recursive rules](tutorial-04-recursive-rules)

Reference: [Bi-temporal queries](datalog-reference#bi-temporal-queries)
