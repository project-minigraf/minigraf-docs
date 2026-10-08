---
title: Tutorial 5: Negation — finding what is absent
nav: 5. Negation
section: Tutorial
order: 5
---
← [Section 4: Recursive rules](tutorial-04-recursive-rules) | → [Section 6: Aggregates and window functions](tutorial-06-aggregates)

---

## Scenario

Clara places a large order for three items: NoiseCancel Pro headphones, a ClearView 27" Monitor, and a USB-C Cable. Corestore picks and ships two of them right away; the third — the cable — sits in the warehouse waiting for a restocking truck. The operations team needs to know which item is still unshipped. More broadly, the catalog team wants to identify products that no customer has ever ordered, and the customer success team wants to flag customers who have never had a completed delivery. All of these questions share a common shape: find things for which some other fact does *not* exist. That is what negation is for.

---

## Data setup (tx 13 and tx 14)

**Cumulative state at the start of this section:** tx_count = 12.

```datalog
; tx 13: Clara places her order — three items
(transact [
  [:clara-order-1        :order/customer          :clara]
  [:clara-order-1        :order/status            :processing]
  [:clara-order-1        :order/delivery-promise  "2026-05-20"]
  [:clara-order-1-item-1 :order-item/order        :clara-order-1]
  [:clara-order-1-item-1 :order-item/product      :nc-headphones]
  [:clara-order-1-item-1 :order-item/price        249]
  [:clara-order-1-item-2 :order-item/order        :clara-order-1]
  [:clara-order-1-item-2 :order-item/product      :monitor-27]
  [:clara-order-1-item-2 :order-item/price        449]
  [:clara-order-1-item-3 :order-item/order        :clara-order-1]
  [:clara-order-1-item-3 :order-item/product      :usb-cable]
  [:clara-order-1-item-3 :order-item/price        19]
])

; tx 14: Two items shipped; Ben's order reaches the customer
(transact [
  [:clara-order-1-item-1 :order-item/shipped      true]
  [:clara-order-1-item-2 :order-item/shipped      true]
  [:ben-order-1          :order/status            :delivered]
  [:ben-order-1          :order/delivery-actual   "2026-05-10"]
])
```

After tx 14, `tx_count` = 14. Items 1 and 2 have an `:order-item/shipped true` fact; item 3 does not. Ben's order now carries the `:delivered` status; Alice's and Clara's orders do not.

---

## `not` — finding the unshipped item

`(not clause...)` excludes an outer binding when all the clauses in the `not` body match for that binding. Every variable appearing inside the `not` body must already be bound by the outer `:where` clauses before `not` is evaluated — this is the **safety rule**.

Here every item in Clara's order is found first (`?item` and `?p` are bound), and then `not` filters out any item that has an `:order-item/shipped true` fact:

```datalog
(query [:find ?product-name
        :where [?item :order-item/order   :clara-order-1]
               [?item :order-item/product ?p]
               [?p    :product/name       ?product-name]
               (not [?item :order-item/shipped true])])
```

```
?product-name
--------------------
"USB-C Cable 2m"

1 result(s) found.
```

`?item` is bound by the first outer clause. The `not` body checks whether that specific `?item` entity has a `:order-item/shipped true` fact. Items 1 and 2 do — they are excluded. Item 3 does not — it passes through.

---

## Products that have never been ordered

To find products that no customer has ordered at any time, you need a variable that is introduced *only* inside the `not` body — the order-item entity `?item` does not exist in the outer scope. This is an **existential variable** and requires `not-join`.

```datalog
(query [:find ?name
        :where [?p   :product/name ?name]
               (not-join [?p]
                         [?item :order-item/product ?p])])
```

```
?name
--------------------
"BudgetBook 14"
"Compact Keyboard"
"PhoneX 11"

3 result(s) found.
```

`not-join [?p]` declares that `?p` is the shared variable bridging outer and inner scope. `?item` is purely existential — it only exists inside the `not-join` body. The query asks: for each product `?p` with a name, is there any order-item entity pointing to it? If no such `?item` exists, the product has never been ordered.

BudgetBook 14, Compact Keyboard, and PhoneX 11 have no order-item facts. The other products — LaptopPro 15 (Ben's order), NoiseCancel Pro, ClearView 27" Monitor, USB-C Cable, and PhoneX 12 — have all appeared in at least one order.

---

## Customers with no completed delivery — `not-join`

The customer success team wants to identify customers who have never had a `:delivered` order. The query needs an inner variable `?order` to join `:order/customer` and `:order/status` — again an existential variable requiring `not-join`:

```datalog
(query [:find ?name
        :where [?customer :customer/name ?name]
               (not-join [?customer]
                         [?order :order/customer ?customer]
                         [?order :order/status   :delivered])])
```

```
?name
--------------------
"Alice"
"Clara"

2 result(s) found.
```

`?customer` is bound by the outer clause and listed in the `not-join` join-vars. `?order` is existential. The engine checks whether there is any order entity that is both owned by `?customer` and has status `:delivered`. Ben's order was marked `:delivered` in tx 14, so Ben is excluded. Alice and Clara have no delivered orders.

---

## `not-join` — the distinction from `not`

`not` and `not-join` share the same core semantics — both exclude outer bindings where the body matches — but they differ in what variables they allow inside the body:

| Form | Join vars | Body variables |
|---|---|---|
| `(not clause...)` | none — all body vars must be pre-bound by outer clauses | only pre-bound variables |
| `(not-join [v1 v2] clause...)` | `v1 v2` — must be pre-bound by outer clauses | `v1 v2` plus any new existential variables |

Use `not` when every variable in the body is already bound. Use `not-join` when you need to introduce new variables inside the body to express an existential condition.

---

## Why the plain `not` form for the delivery query is a parse error

If you tried to write the "customers with no delivery" query using `not` instead of `not-join`, the parser would reject it:

```datalog
; PARSE ERROR — ?order is not pre-bound by any outer clause
(query [:find ?name
        :where [?customer :customer/name ?name]
               (not [?order :order/customer ?customer]
                    [?order :order/status   :delivered])])
```

`?order` appears only inside the `not` body and is not bound by any outer clause. Minigraf enforces the safety rule at parse time: an unbound variable inside `not` is always an error. `not-join` is the correct form here because `?order` is an existential variable.

---

## Multi-clause `not`

`not` can contain multiple clauses as long as every variable in every clause is pre-bound. Here both `?item` and `?p` are bound by the outer clauses before `not` is evaluated:

```datalog
; Items in Clara's order that haven't shipped AND cost less than $50
(query [:find ?product-name ?price
        :where [?item :order-item/order   :clara-order-1]
               [?item :order-item/product ?p]
               [?p    :product/name       ?product-name]
               [?item :order-item/price   ?price]
               (not [?item :order-item/shipped true])
               [(< ?price 50)]])
```

```
?product-name	?price
----------------------------------------
"USB-C Cable 2m"	19

1 result(s) found.
```

The `not` checks a single condition (`?item` has no shipped fact) and the `[(< ?price 50)]` expression filter checks the price. Both conditions must hold for a row to be included. The USB-C Cable ($19) is the only item that is both unshipped and under $50.

---

## Stratification

Minigraf evaluates negation using **stratified Datalog** (Datalog¬). The engine partitions rules into strata based on their dependency graph: a rule that negates a relation is placed in a higher stratum than the rule that defines it, ensuring the negated relation is fully computed before negation is applied.

If you define rules where rule A uses `not` over a relation defined by rule B, and rule B uses `not` over a relation defined by rule A, the engine detects the mutual negation cycle and rejects the second rule at registration time with a clear error message. This is a fundamental restriction of stratified negation — not a Minigraf-specific limitation. Non-recursive negation is always safe and requires no special handling.

---

## Key concepts

- `(not clause...)` — excludes bindings where all clauses in the body match. Every variable in the body must be pre-bound by an outer clause. Parse error if any body variable is unbound.
- `(not-join [v1 v2] clause...)` — `v1 v2` must be pre-bound by outer clauses; additional variables in the body are existential. Use this when you need to introduce new variables inside the not body.
- **Safety rule** — variables inside a `not` body that are not listed in join-vars and not bound by outer clauses cause a parse error.
- **Stratification** — mutual negation cycles between rules are rejected at rule registration time. Non-recursive negation is always safe.

---

← [Section 4: Recursive rules](tutorial-04-recursive-rules) | → [Section 6: Aggregates and window functions](tutorial-06-aggregates)

Reference: [Negation](datalog-reference#negation)
