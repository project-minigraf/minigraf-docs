---
title: Tutorial 2: `:as-of` — Time Travel by Transaction
nav: 2. `:as-of`
section: Tutorial
order: 2
---
← [Section 1: Basic transact + query](tutorial-01-basic-transact-query) | → [Section 3: `:valid-at` and `:any-valid-time`](tutorial-03-valid-at)

---

## Scenario

Ben orders a LaptopPro 15 at $1,299. Two days later the price drops to $1,259. He contacts support: "What was the price shown to me at checkout?" You need to replay the exact database state at the moment he placed his order. `:as-of` makes this a one-liner.

---

## How Minigraf tracks transaction time

Every `transact` or `retract` command increments an internal counter called `tx_count` by 1. This counter is monotonic and auto-managed — you never set it manually. When you write `:as-of 6`, Minigraf filters the database to show only facts that were net-asserted after tx 1 through tx 6, ignoring anything that happened in tx 7 and later. This lets you reconstruct exactly what the database knew at any past moment without storing snapshots.

**Cumulative state at the start of this section:** tx_count = 5
(tx 1: categories, tx 2: products, tx 3: customers, tx 4: Alice's order, tx 5: Alice's USB cable retract)

---

## tx 6: Ben places his order

```datalog
(transact [
  [:ben-order-1        :order/customer :ben]
  [:ben-order-1        :order/status :placed]
  [:ben-order-1-item-1 :order-item/order :ben-order-1]
  [:ben-order-1-item-1 :order-item/product :laptop-pro]
  [:ben-order-1-item-1 :order-item/qty 1]
  [:ben-order-1-item-1 :order-item/price-at-purchase 1299]
])
```

The `:order-item/price-at-purchase` attribute records the price Ben saw at checkout as an explicit fact in the data model. This value belongs to the order, not the product — future product price changes cannot affect it. It is a snapshot written once and never updated.

`:as-of` is the complementary tool: it lets you query the product's own price history as it existed at any past transaction. Together they give you two independent ways to answer "what was the price?": one through your data model, one through time travel.

---

## tx 7 + tx 8: The price drops

To update a value in Minigraf you retract the old fact and assert the new one. A bare `transact` without a preceding `retract` would add a second price fact alongside the first — the product would then have two prices simultaneously.

```datalog
; tx 7: remove the old price
(retract [[:laptop-pro :product/price 1299]])

; tx 8: record the new price
(transact [[:laptop-pro :product/price 1259]])
```

After tx 8, `:laptop-pro` has exactly one active price fact: $1,259.

---

## Query: current price (no `:as-of`)

Without `:as-of`, a query sees all net-asserted facts — the current database state.

```datalog
(query [:find ?price
        :where [:laptop-pro :product/price ?price]])
```

```
?price
--------------------
1259
```

The old $1,299 fact was retracted at tx 7, so it no longer appears.

---

## Query: price as of tx 6

```datalog
(query [:find ?price
        :as-of 6
        :where [:laptop-pro :product/price ?price]])
```

```
?price
--------------------
1299
```

At tx 6, the retraction (tx 7) has not happened yet. The $1,299 fact that was asserted in tx 2 is still net-asserted, so `:as-of 6` returns it. This is the price Ben saw at checkout.

---

## Transaction timeline

```
tx 2  :laptop-pro :product/price 1299  (asserted)
      │
tx 6  Ben places order                 ← :as-of 6 looks here
      │                                  $1,299 is still active
tx 7  :laptop-pro :product/price 1299  (retracted)
tx 8  :laptop-pro :product/price 1259  (asserted)
                                         ↑ current view
```

`:as-of N` draws a vertical line at tx N and shows only facts that are net-asserted up to that point.

**Visualize:** [see this timeline in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#sample=corestore-tutorial&tx=6&vt=any&e=:laptop-pro&view=map) It opens at tx 6, where $1,299 is still the price. Press → to step through the price drop in tx 7 and tx 8.

---

## `:as-of` beyond the latest transaction

If the value of N exceeds the highest tx_count in the database, `:as-of N` returns the current state — there is simply no later data to filter out.

```datalog
; :as-of beyond the latest tx returns the current state
(query [:find ?price
        :as-of 999
        :where [:laptop-pro :product/price ?price]])
```

```
?price
--------------------
1259
```

---

## The purchase price is safe

Even without `:as-of`, the value Ben locked in at order time is always accessible through the order item itself:

```datalog
(query [:find ?recorded-price
        :where [:ben-order-1-item-1 :order-item/price-at-purchase ?recorded-price]])
```

```
?recorded-price
--------------------
1299
```

`:order-item/price-at-purchase` was recorded at tx 6 and never touched again. It is immune to product price changes regardless of how the query is written. `:as-of` answers "what did the database know at tx N?" — `:price-at-purchase` answers "what did we explicitly record in the data model?" Both are valid approaches; use `:price-at-purchase` when the snapshot is a business fact that must survive independently, and `:as-of` when you want to replay history without modifying your schema.

---

## Key concepts

| Concept | Description |
|---|---|
| `:as-of N` | View the database after tx N — transaction time travel |
| Retract + transact | The update pattern in Minigraf; a lone `transact` adds a second fact, not a replacement |
| `:price-at-purchase` | Snapshot value stored in the data model; independent of future product price changes |
| `:as-of` vs snapshot attribute | `:as-of` is a query-time filter; a snapshot attribute is a permanent fact in the graph |

---

← [Section 1: Basic transact + query](tutorial-01-basic-transact-query) | → [Section 3: `:valid-at` and `:any-valid-time`](tutorial-03-valid-at)

Reference: [`:as-of`](datalog-reference#as-of--time-travel-by-transaction-time)
