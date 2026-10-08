---
title: Tutorial 11: Marketplace
nav: 11. Marketplace
section: Tutorial
order: 11
---
← [Section 10: User-Defined Functions](tutorial-10-udfs)

---

## Scenario

Corestore is opening its platform to third-party sellers. In every prior section the store was the only seller, so orders had no seller attribute. Now that multiple sellers can fulfil orders, the data model needs a **seller entity** with its own attributes (name, SLA commitment), and orders must carry a `:order/seller` reference.

This final section shows that all the concepts from Sections 1–10 — pattern clauses, rules, bi-temporal queries, negation, expression filters — compose naturally once a second entity type is added to the graph. No new syntax is required; the marketplace is just another relationship in the EAV store.

---

## Adding sellers to the model

Load these two transactions on top of the cumulative state from Section 10 (tx_count = 21 entering this section):

```datalog
; tx 22: Seller entities
(transact [
  [:corestore-direct :seller/name "Corestore Direct"]
  [:corestore-direct :seller/sla-days 3]
  [:techsource       :seller/name "TechSource"]
  [:techsource       :seller/sla-days 7]
])

; tx 23: Clara's marketplace order via TechSource
(transact [
  [:clara-order-3        :order/customer :clara]
  [:clara-order-3        :order/seller :techsource]
  [:clara-order-3        :order/status :placed]
  [:clara-order-3        :order/delivery-promise "2026-05-28"]
  [:clara-order-3-item-1 :order-item/order :clara-order-3]
  [:clara-order-3-item-1 :order-item/product :laptop-budget]
  [:clara-order-3-item-1 :order-item/price 699]
])
```

After tx 23, `tx_count` = 23.

Two seller entities are asserted in tx 22. Corestore Direct commits to a 3-day SLA; TechSource to 7 days. In tx 23, Clara places a new order (`:clara-order-3`) that references both a customer (`?customer`) and a seller (`?seller`). The order carries a delivery promise date stored as a plain string value.

---

## Querying across seller entities

**Query 1 — Orders with seller SLA > 5 days:**

```datalog
(query [:find ?customer-name ?seller-name ?sla
        :where [?order :order/customer ?customer]
               [?customer :customer/name ?customer-name]
               [?order :order/seller ?seller]
               [?seller :seller/name ?seller-name]
               [?seller :seller/sla-days ?sla]
               [(> ?sla 5)]])
```

```
?customer-name  ?seller-name  ?sla
-----------------------------------
"Clara"         "TechSource"  7

1 result(s) found.
```

The query joins four entity types in a single pattern list: an order, its customer, its seller, and the seller's SLA attribute. The expression clause `[(> ?sla 5)]` filters out Corestore Direct (SLA = 3). Clara is the only customer whose order was placed through TechSource.

See [Pattern clauses](datalog-reference#where-clauses) and [Expression clauses](datalog-reference#arithmetic--predicate-expressions).

---

## A rule for customer–seller relationships

Repeating the four-way join in every query that needs customer–seller pairs is verbose. A rule encapsulates it:

> **Session note:** Rules are not persisted to the `.graph` file. If you start a new REPL session and continue from a saved `corestore.graph`, you must re-define `customer-of-seller` (below) and `subcategory` (from [Section 4](tutorial-04-recursive-rules)) before running any query that uses them.

**Query 2 — Customer–seller rule:**

```datalog
(rule [(customer-of-seller ?customer ?seller)
       [?order :order/customer ?customer]
       [?order :order/seller ?seller]])

(query [:find ?customer-name ?seller-name
        :where (customer-of-seller ?customer ?seller)
               [?customer :customer/name ?customer-name]
               [?seller :seller/name ?seller-name]])
```

```
?customer-name  ?seller-name
------------------------------
"Clara"         "TechSource"

1 result(s) found.
```

The rule `customer-of-seller` abstracts the join through the order entity. The query body can then read as: "find all (customer, seller) pairs where the customer has placed at least one order with that seller." Adding more sellers or reassigning orders later requires no change to the rule definition.

See [Rules](datalog-reference#recursive-rules).

---

## Bi-temporal queries work across sellers

Bi-temporal operators apply to every fact in the store regardless of entity type. The delivery promise recorded in tx 23 is retrievable with `:as-of`:

**Query 3 — What was recorded about Clara's TechSource order?**

```datalog
(query [:find ?promise
        :as-of 23
        :where [:clara-order-3 :order/delivery-promise ?promise]])
```

```
?promise
------------
"2026-05-28"

1 result(s) found.
```

`:as-of 23` pins the transaction horizon to tx_count = 23, which is exactly when Clara's marketplace order was committed. The delivery promise `"2026-05-28"` is returned. If the query were run `:as-of 22` (before tx 23), the fact would not yet exist and the result set would be empty.

See [Bi-temporal queries](datalog-reference#bi-temporal-queries).

---

## Recursive rules across the marketplace graph

Rules compose with marketplace entities exactly as they do with any other entities. The `subcategory` rule from [Section 4](tutorial-04-recursive-rules) traverses the category hierarchy; here it connects marketplace products to their root category:

**Query — marketplace products under Electronics ancestry:**

```datalog
; Re-define the subcategory rule (same as Section 4)
(rule [(subcategory ?parent ?child)
       [?child :category/parent ?parent]])
(rule [(subcategory ?parent ?child)
       [?child :category/parent ?mid]
       (subcategory ?parent ?mid)])

; Find marketplace products under the Electronics category ancestry
(query [:find ?product-name ?seller-name
        :where [?order :order/seller ?seller]
               [?seller :seller/name ?seller-name]
               [?item :order-item/order ?order]
               [?item :order-item/product ?p]
               [?p :product/name ?product-name]
               [?p :product/category ?cat]
               (subcategory :cat-electronics ?cat)])
```

```
?product-name     ?seller-name
-------------------------------
"BudgetBook 14"   "TechSource"

1 result(s) found.
```

BudgetBook 14 is in `:cat-laptops`. The `subcategory` rule finds that `:cat-laptops` is a descendant of `:cat-electronics` (via the direct parent link). The query therefore returns it as a product under the Electronics ancestry, ordered via TechSource.

This is the same recursive fixed-point evaluation introduced in Section 4 — it works across entities of any type, including marketplace sellers and order-items.

See [Rules](datalog-reference#recursive-rules) and [Section 4: Recursive Rules](tutorial-04-recursive-rules).

---

## Sellers with no completed deliveries

Neither seller has delivered an order yet. Negation makes this queryable directly:

**Query 4 — Sellers with no delivered orders:**

```datalog
(query [:find ?seller-name
        :where [?seller :seller/name ?seller-name]
               (not-join [?seller]
                         [?order :order/seller ?seller]
                         [?order :order/status :delivered])])
```

```
?seller-name
------------------
"Corestore Direct"
"TechSource"

2 result(s) found.
```

The `not-join` sub-goal checks, for each seller entity, whether there exists an order via that seller with status `:delivered`. Because no such orders have been asserted yet, both sellers pass the negation filter and appear in the result.

See [Negation](datalog-reference#negation).

---

## Synthesis

The final query combines every major concept from the tutorial series in one expression: cross-entity joins, expression filtering, and item-level detail — all in a single `:find`.

**Query 5 — Items ordered via high-SLA sellers, with prices:**

```datalog
(query [:find ?customer-name ?seller-name ?product-name ?price ?sla
        :where [?order :order/customer ?customer]
               [?customer :customer/name ?customer-name]
               [?order :order/seller ?seller]
               [?seller :seller/name ?seller-name]
               [?seller :seller/sla-days ?sla]
               [(> ?sla 5)]
               [?item :order-item/order ?order]
               [?item :order-item/product ?p]
               [?p :product/name ?product-name]
               [?item :order-item/price ?price]])
```

```
?customer-name  ?seller-name  ?product-name   ?price  ?sla
-----------------------------------------------------------
"Clara"         "TechSource"  "BudgetBook 14"  699     7

1 result(s) found.
```

Starting from the seller SLA filter, the query walks outward through the order to the order item and then to the product entity to retrieve the product name. Every hop is a plain pattern clause — no aggregation or recursion needed for this shape of query. The result confirms that Clara ordered a BudgetBook 14 at price 699 through TechSource (SLA 7 days).

See [Pattern clauses](datalog-reference#where-clauses) and [Expression clauses](datalog-reference#arithmetic--predicate-expressions).

---

## Key concepts

| Concept | Meaning |
|---|---|
| Seller entity | A first-class entity with its own attributes (`:seller/name`, `:seller/sla-days`). Referenced from orders via `:order/seller`. |
| Cross-entity join | A `:where` clause that chains pattern clauses across multiple entity types (order → customer → seller → product). No special syntax required. |
| Rule abstraction | A named rule such as `customer-of-seller` encapsulates repeated join patterns and keeps query bodies readable as the graph grows. |
| `:as-of` across sellers | Bi-temporal operators apply uniformly to all entity types. Marketplace facts are time-travelable in the same way as any other fact. |
| `not-join` for absence | `(not-join [?seller] ...)` tests for the absence of a sub-pattern bound to `?seller`. Works identically whether the entity is a seller, customer, or order. |
| Marketplace pattern | Introduce a seller entity, add `:order/seller` references, and all existing query patterns extend naturally — no schema migration, no DDL. |

---

## Tutorial complete

Sections 1–11 together cover the full Minigraf Datalog grammar: asserting and retracting facts, bi-temporal queries with `:as-of` and `:valid-at`, recursive rules, negation and `not-join`, aggregates and window functions, expression clauses, prepared queries with bind slots, disjunction, user-defined functions, and multi-entity marketplace queries.

For a complete reference to every clause, operator, and built-in function, see the [Datalog Reference](datalog-reference).

---

← [Section 10: User-Defined Functions](tutorial-10-udfs)

References: [Pattern clauses](datalog-reference#where-clauses) · [Rules](datalog-reference#recursive-rules) · [Bi-temporal queries](datalog-reference#bi-temporal-queries) · [Negation](datalog-reference#negation) · [Expression clauses](datalog-reference#arithmetic--predicate-expressions)
