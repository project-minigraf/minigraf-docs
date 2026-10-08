---
title: Tutorial 9: Disjunction — `or` and `or-join`
nav: 9. Disjunction
section: Tutorial
order: 9
---
← [Section 8: Prepared queries](tutorial-08-prepared-queries) | → [Section 10: User-defined functions](tutorial-10-udfs)

---

## Scenario

Corestore's delivery tracking page needs a list of all orders that are either `:shipped` or `:delivered`. A product search page needs to show items from either the Audio or Mobile categories. Clara's refund request covers items from two separate orders — and finding them requires branching on which order an item belongs to. All three of these questions share the same shape: match rows that satisfy *any one of several alternatives*. That is what disjunction (`or` and `or-join`) is for.

---

## Data setup

**Cumulative state at the start of this section:** tx_count ≈ 19.
(tx 1–3: base data; tx 4–5: Alice's order; tx 6–8: Ben's order + price change; tx 9–12: valid-time and inventory data; tx 13–14: Clara's order + Ben's delivery; tx 15–17: Alice's second order + price update; tx 18–19: expression examples)

The delivery tracking examples need clean, single-valued order statuses. Section 5 added `:delivered` to Ben's order without retracting `:placed`, and Clara's order has a stale `:processing` fact. This section retracts those before adding the new data.

```datalog
; Retract stale status facts
(retract [
  [:ben-order-1   :order/status :placed]
  [:clara-order-1 :order/status :processing]
])
```

Now add Clara's second order (cancelled) and advance Clara's first order to `:shipped`:

```datalog
(transact [
  [:clara-order-2        :order/customer :clara]
  [:clara-order-2        :order/status   :cancelled]
  [:clara-order-2-item-1 :order-item/order   :clara-order-2]
  [:clara-order-2-item-1 :order-item/product :keyboard-k1]
  [:clara-order-2-item-1 :order-item/price   89]
  [:clara-order-1        :order/status   :shipped]
])
```

After these two operations, the live order statuses are:

| Order | Customer | Status |
|---|---|---|
| `:alice-order-1` | Alice | `:placed` |
| `:alice-order-2` | Alice | `:placed` |
| `:ben-order-1` | Ben | `:delivered` |
| `:clara-order-1` | Clara | `:shipped` |
| `:clara-order-2` | Clara | `:cancelled` |

---

## `or` — succeed if any branch matches

`(or branch1 branch2 ...)` succeeds for a binding row when *at least one* branch matches. Results from all matching branches are unioned and deduplicated, so each binding row appears at most once.

### Delivery tracking: orders in `:shipped` or `:delivered` status

```datalog
(query [:find ?customer-name ?status
        :where [?order :order/customer ?customer]
               [?customer :customer/name ?customer-name]
               [?order :order/status ?status]
               (or [?order :order/status :shipped]
                   [?order :order/status :delivered])])
```

```
?customer-name  ?status
------------------------
"Clara"         :shipped
"Ben"           :delivered

2 result(s) found.
```

The outer `[?order :order/status ?status]` clause binds `?status` for every status fact on every order. The `or` clause then filters: only keep binding rows where `?order` also has a `:shipped` or `:delivered` status fact. Since each order now has exactly one status, this cleanly produces one row per qualifying order.

> **Note on multiple status facts.** If an order had been left with two status facts — for example `:placed` and `:delivered` both asserted — the outer clause would bind `?status` to both values, and the `or` clause would pass the entire order (because it has `:delivered`). Both `?status` bindings would then survive, including the stale `:placed` one. The fix is always to retract the old status before asserting a new one, as shown in the data setup above.

---

## `or` safety rule — variable parity

All branches of an `or` must introduce the **same set of new variable names**. A variable that already appears in the outer `:where` context is not "new" — it is shared, and different branches can reference it freely. But any *new* variable introduced in one branch must also be introduced in every other branch under the same name.

```datalog
; ERROR — branches introduce different new variables
(or [?order :order/status ?s1]
    [?order :order/status ?s2])
; → Error: [PRS-057] all branches of (or ...) must introduce the same set of new variables

; OK — both branches introduce the same new variable ?s
(or [?order :order/status ?s]
    [?order :order/carrier ?s])

; OK — no new variables introduced; ?order is already bound
(or [?order :order/status :shipped]
    [?order :order/status :delivered])
```

The rule exists because the outer query needs to know, statically, which variables are in scope after the `or` clause. If branches named their outputs differently, the shape of the result set would be ambiguous.

---

## Products in "Audio" or "Mobile" category

This query finds all products that are directly assigned to either the Audio or Mobile category:

```datalog
(query [:find ?name ?category-name
        :where [?p :product/name ?name]
               [?p :product/category ?cat]
               [?cat :category/name ?category-name]
               (or [?cat :category/name "Audio"]
                   [?cat :category/name "Mobile"])])
```

```
?name        ?category-name
----------------------------
"PhoneX 12"  "Mobile"
"PhoneX 11"  "Mobile"

2 result(s) found.
```

Both phones are assigned directly to `:cat-mobile` ("Mobile"). No products are assigned directly to `:cat-audio` ("Audio") — the NoiseCancel Pro sits under `:cat-nc` (Noise-Cancelling), which is a grandchild of Audio. A recursive ancestry query (see [Section 4](tutorial-04-recursive-rules)) would find it, but a direct category check does not.

---

## `(and ...)` inside an `or` branch

A single `or` branch can contain multiple clauses by wrapping them with `(and ...)`. The `and` groups all its clauses into one logical unit that must collectively succeed for the branch to succeed.

```datalog
; Items under $100 in the Accessories category, OR any Mobile phone
(query [:find ?name
        :where [?p :product/name ?name]
               [?p :product/price ?price]
               (or (and [(< ?price 100)]
                        [?p :product/category :cat-accessories])
                   [?p :product/category :cat-mobile])])
```

```
?name
--------------------
"USB-C Cable 2m"
"Compact Keyboard"
"PhoneX 12"
"PhoneX 11"

4 result(s) found.
```

Branch one matches accessories priced below $100: USB-C Cable 2m ($19) and Compact Keyboard ($89). Branch two matches all products in the Mobile category: PhoneX 12 ($799) and PhoneX 11 ($599). The monitor ($449) and laptops are excluded because neither branch covers them.

---

## `or-join` — branches with private variables

`(or-join [join-vars] branch1 branch2 ...)` relaxes the variable parity rule. Only the variables listed in `[join-vars]` are shared with the outer query; any additional variables inside a branch are **private** to that branch and do not need to appear in other branches.

### Finding items from either of Clara's orders

Clara's refund covers items from both `:clara-order-1` and `:clara-order-2`. The two branches reference different order literals, but both produce the same `?item` entity:

```datalog
(query [:find ?product-name ?price
        :where [?item :order-item/product ?p]
               [?p :product/name ?product-name]
               [?item :order-item/price ?price]
               (or-join [?item]
                 [?item :order-item/order :clara-order-1]
                 [?item :order-item/order :clara-order-2])])
```

```
?product-name            ?price
--------------------------------
"NoiseCancel Pro"        249
"USB-C Cable 2m"         19
"ClearView 27" Monitor"  449
"Compact Keyboard"       89

4 result(s) found.
```

`[?item]` in `or-join` declares that only `?item` is shared with the outer query. Each branch checks a specific order entity by literal — no additional join variables are needed. Clara's order-1 contributes the headphones (NoiseCancel Pro), monitor, and cable; order-2 contributes the keyboard.

---

## `or-join` with branch-private variables

When branches navigate through different intermediate entities, each branch's navigation variable can be private:

```datalog
; Items from a shipped order OR from a cancelled order
(query [:find ?product-name
        :where [?item :order-item/product ?p]
               [?p :product/name ?product-name]
               (or-join [?item]
                 (and [?item :order-item/order ?shipped-order]
                      [?shipped-order :order/status :shipped])
                 (and [?item :order-item/order ?cancelled-order]
                      [?cancelled-order :order/status :cancelled]))])
```

```
?product-name
--------------------
"NoiseCancel Pro"
"USB-C Cable 2m"
"ClearView 27" Monitor"
"Compact Keyboard"

4 result(s) found.
```

`?shipped-order` and `?cancelled-order` are private to their respective branches — they do not appear in the `[?item]` join-var list and do not need to match across branches. Only `?item` is propagated to the outer query. Branch one finds items whose order has status `:shipped` (clara-order-1); branch two finds items whose order has status `:cancelled` (clara-order-2). The same four items result.

---

## `or-join` safety rule — join vars must be pre-bound

Every variable listed in `[join-vars]` must already be bound by clauses that appear **before** the `or-join` in the `:where` clause. You cannot introduce a join variable for the first time inside an `or-join` branch.

```datalog
; ERROR — ?item is not bound before the or-join
(query [:find ?product-name
        :where [?p :product/name ?product-name]
               (or-join [?item]
                 [?item :order-item/order :clara-order-1]
                 [?item :order-item/order :clara-order-2])
               [?item :order-item/product ?p]])
; → Error: [INT-018] join variable ?item in (or-join ...) is not bound by any earlier clause

; OK — ?item is bound by a preceding clause
(query [:find ?product-name
        :where [?item :order-item/product ?p]
               [?p :product/name ?product-name]
               (or-join [?item]
                 [?item :order-item/order :clara-order-1]
                 [?item :order-item/order :clara-order-2])])
```

The reason is the same as for `not-join`: the engine needs to evaluate the outer binding before it can enumerate candidates for the existential branches. If a join variable is not yet bound, there is nothing to match inside the branches. Note that the fix above is purely a clause reordering — moving `[?item :order-item/product ?p]` before the `or-join` is the only change required.

---

## `or` inside a rule body

`or` and `or-join` may appear inside a rule body, exactly as they do inside a `:where` clause.

> **Session note:** Rules are not persisted to the `.graph` file. If you start a new REPL session, re-define `active-order` before running any query that uses it.

```datalog
; Define "active" as: placed, processing, or shipped
(rule [(active-order ?customer ?order)
       [?order :order/customer ?customer]
       (or [?order :order/status :placed]
           [?order :order/status :processing]
           [?order :order/status :shipped])])
```

With that rule in place, find all customers who have at least one active order:

```datalog
(query [:find ?customer-name
        :where [?customer :customer/name ?customer-name]
               (active-order ?customer ?order)])
```

```
?customer-name
--------------------
"Clara"
"Alice"
"Alice"

3 result(s) found.
```

Alice appears twice because she has two active orders (`:alice-order-1` and `:alice-order-2`), both in `:placed` status. Clara has one active order (`:clara-order-1`, `:shipped`). Ben's order is `:delivered` — terminal, not active. Clara's order-2 is `:cancelled` — also terminal.

The result has one row per `(customer-name, order)` pair. To collapse Alice's rows into a single customer count, aggregate in `:find`:

```datalog
(query [:find ?customer-name (count ?order)
        :where [?customer :customer/name ?customer-name]
               (active-order ?customer ?order)])
```

```
?customer-name  (count ?order)
--------------------------------
"Alice"         2
"Clara"         1

2 result(s) found.
```

---

## Key concepts

| Concept | Meaning |
|---|---|
| `(or branch1 branch2 ...)` | Succeed if any branch matches. All branches must introduce the same set of new variable names. Results are unioned and deduplicated. |
| `(or-join [vars] branch1 branch2 ...)` | Like `or`, but only `vars` are shared with the outer query. Branch-private variables are existential — they must be bound in the branch, but do not appear outside it. All `vars` must be bound by preceding clauses before `or-join` is evaluated. |
| `(and clause1 clause2 ...)` | Groups multiple clauses into a single `or` / `or-join` branch. All clauses must match for the branch to match. |
| `or` in rule bodies | Allowed. Works identically to `or` in `:where` clauses. |
| Variable parity | Every branch of a plain `or` must introduce the same set of new variable names. `or-join` relaxes this: only the declared join vars need to be consistent; branch-private vars can differ. |

---

← [Section 8: Prepared queries](tutorial-08-prepared-queries) | → [Section 10: User-defined functions](tutorial-10-udfs)

Reference: [Disjunction](datalog-reference#disjunction)
