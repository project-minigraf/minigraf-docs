---
title: Tutorial 4: Recursive rules — traversing graphs of any depth
nav: 4. Recursive rules
section: Tutorial
order: 4
---
← [Section 3: `:valid-at`](tutorial-03-valid-at) | → [Section 5: Negation](tutorial-05-negation)

---

## Scenario

Alice opens the Electronics page and wants to see everything in that section — not just the top-level categories, but every product at any level of nesting. The category tree is four levels deep in places: Electronics → Audio → Headphones → Noise-Cancelling. A flat query can only reach direct children. A recursive rule lets the engine walk the full tree without knowing its shape in advance.

---

## The limit of flat queries

**Cumulative state at the start of this section:** tx_count = 12
(tx 1: categories, tx 2: products, tx 3: customers, tx 4–12: orders and price history from earlier sections)

No new `transact` calls are needed — all data is in the base dataset.

```datalog
; Only direct children of Electronics
(query [:find ?category-name
        :where [?cat :category/parent :cat-electronics]
               [?cat :category/name ?category-name]])
```

```
?category-name
--------------------
"Mobile"
"Laptops"
"Audio"
"Accessories"

4 result(s) found.
```

Only the four direct children are returned. Headphones and Noise-Cancelling are missing because they sit below Audio in the tree — they are grandchildren and great-grandchildren of Electronics, not direct children. A flat pattern clause has no way to follow the `:category/parent` edge more than one step.

---

## Defining a recursive rule

A `rule` defines a named relation. When two (or more) rules share the same head name, their bodies are union-ed together — any row that satisfies at least one body is included. This is how you express base cases and recursive cases:

```datalog
; Base case: a category is a subcategory of its direct parent
(rule [(subcategory ?ancestor ?descendant)
       [?descendant :category/parent ?ancestor]])

; Recursive case: a category is a subcategory of any ancestor's ancestor
(rule [(subcategory ?ancestor ?descendant)
       [?intermediate :category/parent ?ancestor]
       (subcategory ?intermediate ?descendant)])
```

The base case fires for every direct parent-child pair. The recursive case says: if `?intermediate` is a direct child of `?ancestor`, and `?descendant` is (transitively) a subcategory of `?intermediate`, then `?descendant` is also a subcategory of `?ancestor`. The engine evaluates both rules to a fixed point — it keeps applying them until no new tuples are produced. Cycles in the graph are handled correctly; the engine detects when a tuple has already been produced and does not loop.

Rules are invoked in `:where` clauses exactly like fact patterns, using parentheses: `(subcategory :cat-electronics ?cat)`.

---

## All categories under Electronics (any depth)

```datalog
(query [:find ?category-name
        :where (subcategory :cat-electronics ?cat)
               [?cat :category/name ?category-name]])
```

```
?category-name
--------------------
"Mobile"
"Noise-Cancelling"
"Laptops"
"Audio"
"Headphones"
"Accessories"

6 result(s) found.
```

All six descendants are found regardless of depth. Headphones (two hops away) and Noise-Cancelling (three hops away) are now included.

---

## All products under Electronics (via category)

```datalog
(query [:find ?product-name ?category-name
        :where (subcategory :cat-electronics ?cat)
               [?p :product/category ?cat]
               [?p :product/name ?product-name]
               [?cat :category/name ?category-name]])
```

```
?product-name	?category-name
----------------------------------------
"PhoneX 12"	"Mobile"
"USB-C Cable 2m"	"Accessories"
"NoiseCancel Pro"	"Noise-Cancelling"
"BudgetBook 14"	"Laptops"
"Compact Keyboard"	"Accessories"
"LaptopPro 15"	"Laptops"
"PhoneX 11"	"Mobile"

7 result(s) found.
```

**Note:** ClearView 27" Monitor does not appear here. Its category is `:cat-electronics` itself — the root of the subtree we are searching under. `subcategory` finds *descendants* of `:cat-electronics`, not `:cat-electronics` itself. Because the monitor's category is the ancestor, not a descendant, it is excluded. If you want to include products assigned directly to the root category alongside all descendants, add a second `:where` branch or a separate query for `[?p :product/category :cat-electronics]`.

---

## Narrowing to the Audio subtree

The same rule works at any level of the tree. To find only products under Audio:

```datalog
(query [:find ?product-name
        :where (subcategory :cat-audio ?cat)
               [?p :product/category ?cat]
               [?p :product/name ?product-name]])
```

```
?product-name
--------------------
"NoiseCancel Pro"

1 result(s) found.
```

`:cat-audio` has one descendant path: Audio → Headphones → Noise-Cancelling. NoiseCancel Pro is assigned to `:cat-nc`, which is a subcategory of `:cat-audio` via Headphones, so it is the only result.

---

## How fixed-point evaluation works

Minigraf uses **semi-naive fixed-point evaluation** for recursive rules. The engine starts by applying the base-case rule to produce an initial set of tuples — in this example, one tuple per direct parent-child pair. It then applies the recursive rule to those tuples to derive new tuples one level deeper. It applies the rule again to the newly derived tuples, then again, continuing until a full pass produces no new tuples at all. At that point the relation has reached its fixed point and the result is complete. Semi-naive evaluation is an optimisation over naive fixed-point: each iteration only considers tuples that were *newly derived* in the previous iteration, rather than re-examining the entire set. This keeps the cost proportional to the size of the result rather than the square of it. Cycles are safe: once a tuple `(subcategory A B)` is in the set, the engine will not re-derive or re-emit it, so a cycle simply causes the recursive case to match an already-known tuple and produce nothing new, terminating cleanly.

---

## Key concepts

- `rule` defines a named relation with one or more clause bodies. Multiple rules with the same head name define multiple cases (base + recursive), all of which contribute to the relation.
- Rules are invoked in `:where` clauses using parentheses: `(subcategory :cat-electronics ?cat)`.
- Semi-naive fixed-point evaluation handles graphs of any depth and correctly terminates on cyclic graphs.
- A rule's anchor (the entity passed as the first argument) is *excluded* from its own result set — `subcategory` finds descendants, not the root itself.
- **Rules are session-scoped.** Rule definitions are held in memory and are not persisted to the `.graph` file. If you close and reopen the REPL, you must re-define any rules before running queries that use them. Facts survive across sessions; rules do not.

---

← [Section 3: `:valid-at`](tutorial-03-valid-at) | → [Section 5: Negation](tutorial-05-negation)

Reference: [Recursive rules](datalog-reference#recursive-rules)
