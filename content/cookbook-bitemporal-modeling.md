---
title: Bitemporal Modeling
nav: Bitemporal modeling
section: Cookbook
order: 3
---
Recipes for structuring data to take full advantage of Minigraf's bi-temporal model.
These are the "write side" — how to assert facts — complementary to the query patterns
in [Audit and Time-Travel Idioms](cookbook-time-travel).

← [Time-Travel Idioms](cookbook-time-travel) | [Application Workflows →](cookbook-application-workflows)

---

## Recipe 1 — Point-in-time facts (default valid-time)

**Problem:** Assert a fact that becomes true "now" with no modeled expiry.

```datalog
;; Email address recorded at the moment of the transaction
(transact [[:alice :person/email "alice@example.com"]])

;; valid-from = tx_id (wall-clock time of the transaction, Unix ms)
;; valid-to   = 9223372036854775807 (i64::MAX — open-ended)
```

**Notes:**
- No `{:valid-from ...}` map needed; Minigraf sets `valid-from` to the transaction's Unix ms timestamp automatically
- The default query filter (no `:valid-at`) returns these facts as long as they have not been retracted
- Use this for simple current-state storage: contact info, settings, labels — anything that becomes true "now"

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IFtbOmFsaWNlIDpwZXJzb24vZW1haWwgImFsaWNlQGV4YW1wbGUuY29tIl1dKQ&title=Point-in-time+facts&e=:alice&view=map)

---

## Recipe 2 — Bounded facts (known start and end)

**Problem:** Assert a fact that is only true for a specific, fully known time period.

```datalog
;; Annual contract active for all of 2024
(transact {:valid-from "2024-01-01" :valid-to "2024-12-31"}
          [[:contract-99 :contract/status :active]
           [:contract-99 :contract/holder :alice]])

;; Per-fact valid-time map for mixed periods in one transact
(transact [[:policy-a :policy/status :active {:valid-from "2024-01-01" :valid-to "2024-06-30"}]
           [:policy-b :policy/status :active {:valid-from "2024-03-01" :valid-to "2024-12-31"}]])
;; [entity attribute value {:valid-from ... :valid-to ...}]
```

**Notes:**
- Transaction-level `{:valid-from ... :valid-to ...}` applies to all facts in the batch
- A per-fact `{:valid-from ... :valid-to ...}` map (the optional 4th element) overrides the transaction-level range for that fact
- Bounded facts model contracts, subscriptions, approvals, and any period with a known start and end

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyNC0wMS0wMSIgOnZhbGlkLXRvICIyMDI0LTEyLTMxIn0KICAgICAgICAgIFtbOmNvbnRyYWN0LTk5IDpjb250cmFjdC9zdGF0dXMgOmFjdGl2ZV0KICAgICAgICAgICBbOmNvbnRyYWN0LTk5IDpjb250cmFjdC9ob2xkZXIgOmFsaWNlXV0pCih0cmFuc2FjdCBbWzpwb2xpY3ktYSA6cG9saWN5L3N0YXR1cyA6YWN0aXZlIHs6dmFsaWQtZnJvbSAiMjAyNC0wMS0wMSIgOnZhbGlkLXRvICIyMDI0LTA2LTMwIn1dCiAgICAgICAgICAgWzpwb2xpY3ktYiA6cG9saWN5L3N0YXR1cyA6YWN0aXZlIHs6dmFsaWQtZnJvbSAiMjAyNC0wMy0wMSIgOnZhbGlkLXRvICIyMDI0LTEyLTMxIn1dXSk&title=Bounded+facts&vt=2024-04-01&view=map)

---

## Recipe 3 — Open-ended current facts (known start, no end)

**Problem:** Assert a fact that started on a known date and is still true today.

```datalog
;; Employment beginning on a specific date, no known end date
(transact {:valid-from "2023-06-01"}
          [[:alice :works-at :startupco]])
;; valid-from = 2023-06-01; valid-to = i64::MAX (open-ended)

;; Query: who currently works at :startupco?
(query [:find ?person
        :where [?person :works-at :startupco]])
;; Returns :alice — the default filter sees open-ended facts as currently valid ✓
```

**Notes:**
- Omitting `:valid-to` leaves the fact open-ended (valid forever from `valid-from`)
- The default query filter (no `:valid-at`) returns facts where `valid-from ≤ now`; an open-ended fact set in the past is visible
- To close this fact when the situation changes, use Recipe 7

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyMy0wNi0wMSJ9CiAgICAgICAgICBbWzphbGljZSA6d29ya3MtYXQgOnN0YXJ0dXBjb11dKQ&title=Open-ended+current+facts&e=:alice&view=map)

---

## Recipe 4 — Retroactive correction

**Problem:** Correct a fact that was recorded with a wrong value, while preserving the original erroneous record in history.

```datalog
;; Wrong salary was recorded
(transact {:valid-from "2024-01-01"}
          [[:alice :salary 75000]])  ;; tx 1 — incorrect

;; Correction: Alice's actual salary was 80000 from the same date
(retract [[:alice :salary 75000]])   ;; tx 2 — marks old value as retracted
(transact {:valid-from "2024-01-01"}
          [[:alice :salary 80000]])  ;; tx 3 — correct value, same valid-from

;; Current state: corrected salary
(query [:find ?s :where [:alice :salary ?s]])
;; => 80000

;; History preserved: original wrong value visible at tx 1
(query [:find ?s
        :as-of 1
        :valid-at :any-valid-time
        :where [:alice :salary ?s]])
;; => 75000
```

**Notes:**
- `retract` marks the fact as no longer asserted; it does **not** delete it from history
- The re-asserted fact carries the **same `valid-from`** as the original — this is what makes it retroactive
- The full audit trail (wrong value at tx 1, correction at tx 3) is always preserved and queryable

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyNC0wMS0wMSJ9CiAgICAgICAgICBbWzphbGljZSA6c2FsYXJ5IDc1MDAwXV0pCihyZXRyYWN0IFtbOmFsaWNlIDpzYWxhcnkgNzUwMDBdXSkKKHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyNC0wMS0wMSJ9CiAgICAgICAgICBbWzphbGljZSA6c2FsYXJ5IDgwMDAwXV0p&title=Retroactive+correction&tx=3&vt=any&e=:alice&view=map)

---

## Recipe 5 — Future-dated assertion

**Problem:** Record a change that takes effect at a future date.

```datalog
;; Promotion effective 2026-01-01, recorded today
(transact {:valid-from "2026-01-01"}
          [[:alice :title :senior-engineer]])

;; Query today (before 2026-01-01): title is not yet visible
(query [:find ?title :where [:alice :title ?title]])
;; => empty

;; Query at or after 2026-01-01
(query [:find ?title
        :valid-at "2026-01-01"
        :where [:alice :title ?title]])
;; => :senior-engineer
```

**Notes:**
- The fact is stored immediately but invisible to queries without an explicit `:valid-at` (since `valid-from > now`)
- Useful for scheduling promotions, pre-approvals, contract renewals, and "effective date" workflows
- To list all future-dated facts: query with `:valid-at :any-valid-time` and filter `[(> ?vf <now-ms>)]` using `:db/valid-from`

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyNi0wMS0wMSJ9CiAgICAgICAgICBbWzphbGljZSA6dGl0bGUgOnNlbmlvci1lbmdpbmVlcl1dKQ&title=Future-dated+assertion&vt=2026-01-01&e=:alice&view=map)

---

## Recipe 6 — Modeling overlapping valid periods

**Problem:** Represent a situation where an entity holds two concurrent values for the same attribute.

```datalog
;; Alice holds two roles simultaneously during 2024
(transact {:valid-from "2024-01-01" :valid-to "2024-06-30"}
          [[:alice :role :project-lead]])
(transact {:valid-from "2024-03-01"}
          [[:alice :role :technical-advisor]])

;; Query on 2024-04-15: both roles are active
(query [:find ?role
        :valid-at "2024-04-15"
        :where [:alice :role ?role]])
;; => :project-lead, :technical-advisor
```

**Notes:**
- Minigraf places no uniqueness constraint on attribute values across valid-time ranges — overlapping periods are fully supported
- To enforce "at most one active value at a time", validate at the application layer before transacting
- Use `:valid-at :any-valid-time` to see all periods including non-overlapping historical ones
<!-- @since v3.0.0 -->
- The two periods here are different values, so they are different facts. One fact (the same entity, attribute *and* value) has only one current window: asserting `[:alice :role :project-lead]` again with another window replaces the first. To record the same value over two separate periods, give each period its own entity (for example an `:assignment` entity with `:assignment/role` and its own window), or read the earlier period with `:as-of`.
<!-- @end -->

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyNC0wMS0wMSIgOnZhbGlkLXRvICIyMDI0LTA2LTMwIn0KICAgICAgICAgIFtbOmFsaWNlIDpyb2xlIDpwcm9qZWN0LWxlYWRdXSkKKHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyNC0wMy0wMSJ9CiAgICAgICAgICBbWzphbGljZSA6cm9sZSA6dGVjaG5pY2FsLWFkdmlzb3JdXSk&title=Overlapping+valid+periods&vt=2024-04-15&e=:alice&view=map)

---

## Recipe 7 — Closing an open-ended fact

**Problem:** Record the real-world end of a situation that was modeled as open-ended.

<!-- @until v3.0.0 -->
```datalog
;; Alice left StartupCo on 2025-12-31
;; Step 1: retract the open-ended fact
(retract [[:alice :works-at :startupco]])

;; Step 2: re-assert with an explicit valid-to
(transact {:valid-from "2023-06-01" :valid-to "2025-12-31"}
          [[:alice :works-at :startupco]])
```

**Notes:**
- Minigraf facts are immutable; you cannot add `valid-to` to an existing open-ended fact in place
- Retract + re-assert is the correct pattern; the original open-ended fact is preserved in history (visible via `:as-of <tx before retraction>`)
- The re-asserted fact carries the **original `valid-from`** so the full employment period (2023-06-01 to 2025-12-31) is correctly modeled
- Follow immediately with a new open-ended fact if Alice starts a new role: `(transact {:valid-from "2026-01-15"} [[:alice :works-at :nextcorp]])`

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyMy0wNi0wMSJ9CiAgICAgICAgICBbWzphbGljZSA6d29ya3MtYXQgOnN0YXJ0dXBjb11dKQoocmV0cmFjdCBbWzphbGljZSA6d29ya3MtYXQgOnN0YXJ0dXBjb11dKQoodHJhbnNhY3Qgezp2YWxpZC1mcm9tICIyMDIzLTA2LTAxIiA6dmFsaWQtdG8gIjIwMjUtMTItMzEifQogICAgICAgICAgW1s6YWxpY2UgOndvcmtzLWF0IDpzdGFydHVwY29dXSk&title=Closing+an+open-ended+fact&tx=3&e=:alice&view=map)
<!-- @end -->
<!-- @since v3.0.0 -->
```datalog
;; Alice joined StartupCo on 2023-06-01 (open-ended)
(transact {:valid-from "2023-06-01"}
          [[:alice :works-at :startupco]])

;; She left on 2025-12-31: assert the same fact with the closed window
(transact {:valid-from "2023-06-01" :valid-to "2025-12-31"}
          [[:alice :works-at :startupco]])
```

**Notes:**
- The later assertion replaces the fact's window, so after it the fact is valid only from 2023-06-01 to 2025-12-31. The open-ended window stays visible through `:as-of` the first transaction.
- No `retract` is needed. `retract` means "this fact is no longer asserted at all"; closing a window is a `transact` with the new bounds. Extending or reopening a window works the same way.
- The closing assertion carries the **original `valid-from`** so the full employment period is modeled.
- Follow with a new open-ended fact if Alice starts a new role: `(transact {:valid-from "2026-01-15"} [[:alice :works-at :nextcorp]])`
<!-- @end -->

---

## Recipe 8 — Correction vs. lifecycle end

**Problem:** Know when to use error correction (Recipe 4) vs. valid-time bounding (Recipe 7).

<!-- @until v3.0.0 -->
```datalog
;; ERROR CORRECTION — the original value was WRONG
;; Preserve the original valid-from in the replacement
(retract [[:alice :salary 75000]])
(transact {:valid-from "2024-01-01"}  ;; same start date as the wrong fact
          [[:alice :salary 80000]])

;; LIFECYCLE END — the original value was CORRECT but the situation changed
;; Close the fact at the real-world end date; add a new fact for the next period
(retract [[:alice :works-at :startupco]])
(transact {:valid-from "2023-06-01" :valid-to "2025-12-31"}
          [[:alice :works-at :startupco]])
(transact {:valid-from "2026-01-15"}
          [[:alice :works-at :nextcorp]])
```

**Notes:**
- The distinction is semantic, not mechanical — both use retract + re-assert
- **Error correction:** preserve the original `valid-from` in the replacement; the wrong value was never correct in the real world
- **Lifecycle end:** close the fact at the real-world end date; assert a new fact for the new period
- Both preserve the full history of what was recorded and when; `:as-of` lets you see the database state before either change

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyNC0wMS0wMSJ9IFtbOmFsaWNlIDpzYWxhcnkgNzUwMDBdXSkKKHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyMy0wNi0wMSJ9IFtbOmFsaWNlIDp3b3Jrcy1hdCA6c3RhcnR1cGNvXV0pCihyZXRyYWN0IFtbOmFsaWNlIDpzYWxhcnkgNzUwMDBdXSkKKHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyNC0wMS0wMSJ9ICA7OyBzYW1lIHN0YXJ0IGRhdGUgYXMgdGhlIHdyb25nIGZhY3QKICAgICAgICAgIFtbOmFsaWNlIDpzYWxhcnkgODAwMDBdXSkKKHJldHJhY3QgW1s6YWxpY2UgOndvcmtzLWF0IDpzdGFydHVwY29dXSkKKHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyMy0wNi0wMSIgOnZhbGlkLXRvICIyMDI1LTEyLTMxIn0KICAgICAgICAgIFtbOmFsaWNlIDp3b3Jrcy1hdCA6c3RhcnR1cGNvXV0pCih0cmFuc2FjdCB7OnZhbGlkLWZyb20gIjIwMjYtMDEtMTUifQogICAgICAgICAgW1s6YWxpY2UgOndvcmtzLWF0IDpuZXh0Y29ycF1dKQ&title=Correction+vs.+lifecycle+end&vt=any&e=:alice&view=map)
<!-- @end -->
<!-- @since v3.0.0 -->
```datalog
;; ERROR CORRECTION — the original value was WRONG
;; Withdraw it, and assert the right value with the original valid-from
(retract [[:alice :salary 75000]])
(transact {:valid-from "2024-01-01"}  ;; same start date as the wrong fact
          [[:alice :salary 80000]])

;; LIFECYCLE END — the original value was CORRECT but the situation changed
;; Close the fact's window at the real-world end date; add a new fact for the next period
(transact {:valid-from "2023-06-01" :valid-to "2025-12-31"}
          [[:alice :works-at :startupco]])
(transact {:valid-from "2026-01-15"}
          [[:alice :works-at :nextcorp]])
```

**Notes:**
- **Error correction** retracts: the wrong value was never true, so it is withdrawn, and the right value is asserted with the original `valid-from`.
- **Lifecycle end** does not retract: the value was true, so the same fact is asserted again with its closed window, and a new fact covers the new period.
- Both preserve the full history of what was recorded and when; `:as-of` lets you see the database state before either change
<!-- @end -->

---

← [Time-Travel Idioms](cookbook-time-travel) | [Application Workflows →](cookbook-application-workflows)

Reference: [Bi-temporal queries](datalog-reference#bi-temporal-queries), [Transact](datalog-reference#transact), [Retract](datalog-reference#retract)
