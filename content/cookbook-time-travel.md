---
title: Audit and Time-Travel Idioms
nav: Time travel
section: Cookbook
order: 2
---
Recipes for querying Minigraf's bi-temporal history. Organised by the four temporal query
classes from the [RecallGraph taxonomy](https://recallgraph.hashnode.dev/temporal-query-types).

← [Graph Traversal](cookbook-graph-traversal) | [Bitemporal Modeling →](cookbook-bitemporal-modeling)

---

## Point-in-Time queries

A point-in-time query returns the state of the database at a single frozen moment. Both time
axes (transaction time and valid time) support point-in-time queries independently or combined.

---

### Recipe 1 — Transaction-time snapshot by tx count

**Problem:** Query the database exactly as it existed after transaction N.

```datalog
;; Config table updated three times
(transact [[:cfg :app/max-conn 10]])   ;; tx 1
(transact [[:cfg :app/max-conn 50]])   ;; tx 2
(transact [[:cfg :app/max-conn 100]])  ;; tx 3

;; What was max-conn after tx 1?
(query [:find ?v
        :as-of 1
        :valid-at :any-valid-time
        :where [:cfg :app/max-conn ?v]])
;; => 10
```

**Notes:**
- `:as-of N` uses the sequential tx counter (1, 2, 3…), not the Unix ms tx-id shown by the REPL's "Transacted (tx: …)" output
- Each `(transact [...])` increments the counter once regardless of how many facts it contains
- `:valid-at :any-valid-time` suppresses the default valid-time filter; omit it only if your facts carry explicit valid-time ranges that you want applied

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IFtbOmNmZyA6YXBwL21heC1jb25uIDEwXV0pCih0cmFuc2FjdCBbWzpjZmcgOmFwcC9tYXgtY29ubiA1MF1dKQoodHJhbnNhY3QgW1s6Y2ZnIDphcHAvbWF4LWNvbm4gMTAwXV0p&title=Transaction-time+snapshot&tx=1&vt=any&e=:cfg&view=map)

---

### Recipe 2 — Transaction-time snapshot by wall-clock time

**Problem:** Query the database as it existed at a specific wall-clock timestamp.

```datalog
(query [:find ?v
        :as-of "2025-06-01T12:00:00Z"
        :valid-at :any-valid-time
        :where [:cfg :app/max-conn ?v]])
```

**Notes:**
- Matches the latest transaction whose `tx_id` (Unix ms) is ≤ the given timestamp
- Timestamps must be ISO 8601 UTC; Minigraf rejects local-timezone or offset timestamps
- If no transaction existed before the given timestamp, the query returns empty

---

### Recipe 3 — Valid-time snapshot (standalone `:valid-at`)

**Problem:** Query what was true in the real world on a specific date, regardless of when it was recorded.

```datalog
;; Employment history with explicit valid-time ranges
(transact {:valid-from "2020-01-01" :valid-to "2023-06-01"}
          [[:alice :works-at :techcorp]])
(transact {:valid-from "2023-06-01"}
          [[:alice :works-at :startupco]])

;; Where did Alice work on 2021-07-01?
(query [:find ?company
        :valid-at "2021-07-01"
        :where [:alice :works-at ?company]])
;; => :techcorp

;; Where does Alice work today?
(query [:find ?company
        :where [:alice :works-at ?company]])
;; => :startupco  (no :valid-at = currently valid facts only)
```

**Notes:**
- `:valid-at "date"` filters to facts where `valid-from ≤ date < valid-to`
- Omitting `:valid-at` returns only facts currently valid (`valid-to = forever` and `valid-from ≤ now`)
- `:valid-at :any-valid-time` returns all facts regardless of valid-time — use this when you need the full history

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyMC0wMS0wMSIgOnZhbGlkLXRvICIyMDIzLTA2LTAxIn0KICAgICAgICAgIFtbOmFsaWNlIDp3b3Jrcy1hdCA6dGVjaGNvcnBdXSkKKHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyMy0wNi0wMSJ9CiAgICAgICAgICBbWzphbGljZSA6d29ya3MtYXQgOnN0YXJ0dXBjb11dKQ&title=Valid-time+snapshot&vt=2021-07-01&e=:alice&view=map)

---

### Recipe 4 — Bi-temporal snapshot

**Problem:** Ask "what did the database *know* at transaction N about what was *true* on date D?"

```datalog
;; As-of tx 1 (only TechCorp recorded): where was Alice in 2024?
(query [:find ?company
        :as-of 1
        :valid-at "2024-01-01"
        :where [:alice :works-at ?company]])
;; => empty — TechCorp expired before 2024; StartupCo not yet in DB at tx 1

;; As-of tx 2 (both recorded): where was Alice in 2024?
(query [:find ?company
        :as-of 2
        :valid-at "2024-01-01"
        :where [:alice :works-at ?company]])
;; => :startupco
```

**Notes:**
- `:as-of` controls what was *recorded*; `:valid-at` controls what was *true in the real world*
- Use this for auditing decisions: "what information did we have available when we made choice X?"
- Both axes are fully independent; combining them narrows the result to the intersection

**Visualize:** [Open Recipe 3's data at `:as-of 1`, `:valid-at 2024-01-01` in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyMC0wMS0wMSIgOnZhbGlkLXRvICIyMDIzLTA2LTAxIn0KICAgICAgICAgIFtbOmFsaWNlIDp3b3Jrcy1hdCA6dGVjaGNvcnBdXSkKKHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyMy0wNi0wMSJ9CiAgICAgICAgICBbWzphbGljZSA6d29ya3MtYXQgOnN0YXJ0dXBjb11dKQ&title=Bi-temporal+snapshot&tx=1&vt=2024-01-01&e=:alice&view=map)

---

## Time Interval queries

A time interval query gathers data across a range of the timeline rather than at a single point.

---

### Recipe 5 — Full history of an attribute

**Problem:** List every value ever recorded for an entity/attribute pair, including retracted values.

```datalog
;; All salary values ever recorded for :alice (including retracted ones)
(transact [[:alice :salary 75000]])  ;; tx 1
(retract  [[:alice :salary 75000]])
(transact [[:alice :salary 80000]])  ;; tx 3

(query [:find ?salary ?tx
        :valid-at :any-valid-time
        :where [:alice :salary ?salary]
               [:alice :db/tx-count ?tx]])
;; => [(75000, 1), (80000, 3)]
```

**Notes:**
- `:valid-at :any-valid-time` with no `:as-of` returns all facts ever asserted, including retracted ones
- `[:alice :db/tx-count ?tx]` binds the tx-count of each salary fact — the pseudo-attribute correlates with the preceding real-attribute pattern on the same entity
- Sort by `?tx` to read changes in chronological order

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IFtbOmFsaWNlIDpzYWxhcnkgNzUwMDBdXSkKKHJldHJhY3QgIFtbOmFsaWNlIDpzYWxhcnkgNzUwMDBdXSkKKHRyYW5zYWN0IFtbOmFsaWNlIDpzYWxhcnkgODAwMDBdXSk&title=Full+history+of+an+attribute&vt=any&e=:alice&view=map)

---

### Recipe 6 — Changes between two tx counts

**Problem:** Find all values asserted for an attribute within a specific transaction range.

```datalog
;; Price changes between tx 3 and tx 7
(query [:find ?price ?tx
        :valid-at :any-valid-time
        :where [:product-x :product/price ?price]
               [:product-x :db/tx-count   ?tx]
               [(>= ?tx 3)]
               [(<= ?tx 7)]])
```

**Notes:**
- `?tx` is an integer (sequential counter); compare with `>=` and `<=` expression predicates
- To find changes across *all* entities in a tx range, replace the entity with `?e`: `[?e :product/price ?price] [?e :db/tx-count ?tx]`
- Combine with `:db/tx-id ?ti` to get the wall-clock time of each change: `[(/ ?ti 1000) ?unix-sec]`

---

### Recipe 7 — Facts valid during a date range (VT overlap)

**Problem:** Find all facts that were valid at any point during a given date range.

```datalog
;; Insurance policies valid at any point during 2023
;; Overlap condition: fact started before range-end AND fact ended after range-start

;; 2023-01-01 = 1672531200000 ms  |  2024-01-01 = 1704067200000 ms
(query [:find ?policy ?name
        :valid-at :any-valid-time
        :where [?policy :policy/name  ?name]
               [?policy :db/valid-from ?vf]
               [?policy :db/valid-to   ?vt]
               [(< ?vf 1704067200000)]   ;; started before end of range
               [(>= ?vt 1672531200000)]])  ;; ended on or after start of range
;; => all policies with any overlap with 2023
```

**Notes:**
- Valid-time values are stored as Unix milliseconds; pre-compute range boundaries before querying
- Open-ended facts have `valid-to = 9223372036854775807` (i64::MAX) and always satisfy `>= range_start`
- The overlap condition is: `fact_start < range_end AND fact_end >= range_start`

---

## Time-Point Lookup queries

A time-point lookup finds the point(s) on the timeline where given criteria were satisfied —
the inverse of point-in-time.

---

### Recipe 8 — When was X first asserted?

**Problem:** Find the transaction at which a specific fact was first recorded.

```datalog
;; When was :alice's name first recorded?
(query [:find (min ?tx)
        :valid-at :any-valid-time
        :where [:alice :person/name _]
               [:alice :db/tx-count ?tx]])
;; => the earliest tx count for any name fact for :alice
```

**Notes:**
- `(min ?tx)` returns the earliest; `(max ?tx)` returns the most recent assertion
- To get the wall-clock time instead: replace `:db/tx-count` with `:db/tx-id` — the value is Unix milliseconds
- To find the first assertion across all entities for an attribute, use `?e` instead of `:alice` and add `?e` to `:find`

---

### Recipe 9 — When did X become valid?

**Problem:** Find the valid-time at which a specific fact started being true in the real world.

```datalog
;; When did Alice's employment at TechCorp begin (valid-time)?
(query [:find ?vf
        :valid-at :any-valid-time
        :where [:alice :works-at :techcorp]
               [:alice :db/valid-from ?vf]])
;; => Unix ms timestamp corresponding to 2020-01-01
```

**Notes:**
- `[:alice :db/valid-from ?vf]` binds the `valid-from` of the fact matched by the preceding pattern
- If the fact was asserted without an explicit `valid-from`, the value equals `tx_id` (the wall-clock time of the transaction)
- Divide the result by 1000 to get Unix seconds; divide by 86400000 for days since epoch

---

### Recipe 10 — When did X expire?

**Problem:** Find the valid-time at which a specific fact stopped being true.

```datalog
;; When did Alice's TechCorp employment end?
(query [:find ?vt
        :valid-at :any-valid-time
        :where [:alice :works-at :techcorp]
               [:alice :db/valid-to ?vt]
               [(< ?vt 9223372036854775807)]])  ;; exclude open-ended facts
;; => Unix ms timestamp corresponding to 2023-06-01
```

**Notes:**
- `9223372036854775807` is `i64::MAX` — the sentinel for "valid forever"; filter it out to find only expired facts
- Retracted facts may carry `valid-to = i64::MAX` at the moment of retraction; use `:as-of` to distinguish retracted-then-replaced from genuinely expired
- To find all facts that expired before a given date: add `[(<= ?vt <date-ms>)]`

---

## Time-Interval Lookup queries

A time-interval lookup finds the interval(s) during which given criteria held — the inverse of time-interval.

---

### Recipe 11 — Duration of a valid period

**Problem:** Compute how long a specific fact was continuously valid.

```datalog
;; Duration of Alice's TechCorp employment in milliseconds
(query [:find ?duration
        :valid-at :any-valid-time
        :where [:alice :works-at :techcorp]
               [:alice :db/valid-from ?vf]
               [:alice :db/valid-to   ?vt]
               [(< ?vt 9223372036854775807)]  ;; exclude open-ended
               [(- ?vt ?vf) ?duration]])
;; => duration in ms; divide by 86400000 for days
```

**Notes:**
- `[(- ?vt ?vf) ?duration]` is an arithmetic binding — it computes and binds the result
- Filter open-ended facts (`valid-to = i64::MAX`) first; subtracting i64::MAX overflows
- For open-ended facts, substitute the current Unix ms timestamp as the upper bound before computing duration

---

### Recipe 12 — All valid periods for an attribute

**Problem:** List every time interval during which a given entity/attribute pair held a value.

```datalog
;; All employment periods for :alice (all employers, all intervals)
(query [:find ?company ?vf ?vt
        :valid-at :any-valid-time
        :where [:alice :works-at ?company]
               [:alice :db/valid-from ?vf]
               [:alice :db/valid-to   ?vt]])
;; => [(:techcorp, <vf-ms>, <vt-ms>), (:startupco, <vf-ms>, 9223372036854775807)]
```

**Notes:**
- Results include all historical intervals; open-ended ones show `valid-to = 9223372036854775807`
- Order by `?vf` to read the history chronologically
- Add `[(< ?vt 9223372036854775807)]` to show only closed (expired) periods; remove it to include the current open interval

**Visualize:** [See every valid period of Recipe 3's data on the bitemporal map →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyMC0wMS0wMSIgOnZhbGlkLXRvICIyMDIzLTA2LTAxIn0KICAgICAgICAgIFtbOmFsaWNlIDp3b3Jrcy1hdCA6dGVjaGNvcnBdXSkKKHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyMy0wNi0wMSJ9CiAgICAgICAgICBbWzphbGljZSA6d29ya3MtYXQgOnN0YXJ0dXBjb11dKQ&title=All+valid+periods+for+an+attribute&vt=any&e=:alice&view=map)

---

## Supporting patterns

---

### Recipe 13 — Audit trail with actor

**Problem:** Record which actor made each change, and later find all changes made by a specific actor.

```datalog
;; Transact actor metadata alongside the data change — same tx-count ties them together
(transact [[:tx-meta-1 :tx/actor  "alice@example.com"]
           [:tx-meta-1 :tx/reason "quarterly-review"]
           [:alice      :salary    80000]])

;; Find all salary changes made by alice@example.com
(query [:find ?entity ?salary ?tx
        :valid-at :any-valid-time
        :where [?meta :tx/actor "alice@example.com"]
               [?meta :db/tx-count ?tx]
               [?entity :salary ?salary]
               [?entity :db/tx-count ?tx]])
;; => [(:alice, 80000, N)] — all salary facts recorded in the same tx as the actor fact
```

**Notes:**
- Correlate the actor entity's `:db/tx-count` with data entities' `:db/tx-count` to find all facts written in the same transaction
- Use a naming convention for tx-meta entities (e.g., `:tx-meta-<uuid>`) to avoid collisions
- Actor facts are themselves bitemporal — query them with `:as-of` to reconstruct who was responsible for a change at a past tx

---

← [Graph Traversal](cookbook-graph-traversal) | [Bitemporal Modeling →](cookbook-bitemporal-modeling)

Reference: [Bi-temporal queries](datalog-reference#bi-temporal-queries), [Pseudo-attributes](datalog-reference#any-valid-time--ignore-valid-time-filter)
