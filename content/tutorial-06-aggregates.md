---
title: Tutorial 6: Aggregates, `:with`, and window functions
nav: 6. Aggregates & windows
section: Tutorial
order: 6
---
← [Section 5: Negation](tutorial-05-negation) | → [Section 7: Expression clauses](tutorial-07-expressions)

---

## Scenario

The Corestore analytics team has three questions. First: how much has each customer spent in total? Second: what is the cheapest product in each category? Third: which products rank highest by price within their category? These questions have different shapes. The first two collapse many rows into a single summary row per group — that is what **scalar aggregates** do. The third annotates each row with a computed rank without changing the number of rows — that is what **window functions** do.

This section also covers `:with`, a clause that controls grouping granularity and is essential for queries where you want per-item contributions without merging rows that share the same projected values.

---

## Data setup (tx 15 through tx 17)

**Cumulative state at the start of this section:** tx_count = 14.

```datalog
; tx 15: Alice's second order — two identical USB-C cables
(transact [
  [:alice-order-2        :order/customer :alice]
  [:alice-order-2        :order/status :placed]
  [:alice-order-2-item-1 :order-item/order :alice-order-2]
  [:alice-order-2-item-1 :order-item/product :usb-cable]
  [:alice-order-2-item-1 :order-item/qty 1]
  [:alice-order-2-item-1 :order-item/price 19]
  [:alice-order-2-item-2 :order-item/order :alice-order-2]
  [:alice-order-2-item-2 :order-item/product :usb-cable]
  [:alice-order-2-item-2 :order-item/qty 1]
  [:alice-order-2-item-2 :order-item/price 19]
])

; tx 16: Retract the current LaptopPro price
(retract [[:laptop-pro :product/price 1259]])

; tx 17: New lower LaptopPro price
(transact [[:laptop-pro :product/price 1229]])
```

Alice's second order contains two separate line-item entities — `:alice-order-2-item-1` and `:alice-order-2-item-2` — both pointing to the same product (`:usb-cable`) at the same price ($19). This is intentional: the order is for two physical cables. After tx 17, LaptopPro 15 carries a new price of $1,229.

After tx 17, `tx_count` = 17.

---

## Scalar aggregates

Scalar aggregate functions appear in the `:find` clause alongside plain variables. Plain variables become **grouping keys** — one output row is produced per unique combination of their values. Aggregate functions compute over all rows in each group.

### How grouping works

When `:find` contains a mix of plain variables and aggregate calls, Minigraf groups result rows by the plain variables and applies the aggregates within each group. A query with only aggregate calls (no plain variables) produces a single row.

```
:find ?a ?b (sum ?c)
```

Groups by `(?a, ?b)`. All rows with the same `?a` and `?b` values are merged; `(sum ?c)` adds their `?c` values.

### Available aggregates

| Function | Description |
|---|---|
| `(count ?x)` | Total number of rows in the group |
| `(count-distinct ?x)` | Number of distinct values of `?x` in the group |
| `(sum ?x)` | Sum of all numeric values of `?x` |
| `(sum-distinct ?x)` | Sum of distinct numeric values of `?x` |
| `(min ?x)` | Minimum value (numbers and strings) |
| `(max ?x)` | Maximum value (numbers and strings) |

All aggregates silently skip `null` values. `count` and `count-distinct` return `0` when the group is empty; other aggregates return no row for an empty group.

---

## Total spend per customer

This query finds total spend by grouping on `?customer-name` — all order items for the same customer collapse into one group:

```datalog
(query [:find ?customer-name (sum ?price)
        :where [?customer :customer/name ?customer-name]
               [?order :order/customer ?customer]
               [?item :order-item/order ?order]
               [?item :order-item/price ?price]])
```

```
?customer-name	(sum ?price)
----------------------------------------
"Alice"	837
"Clara"	717

2 result(s) found.
```

Alice's $837 breaks down as: order 1 item 1 (PhoneX 12, $799) + order 2 item 1 (USB-C Cable, $19) + order 2 item 2 (USB-C Cable, $19).

Clara's $717: NoiseCancel Pro ($249) + ClearView 27" Monitor ($449) + USB-C Cable ($19).

Ben does not appear because his order uses `:order-item/price-at-purchase`, not `:order-item/price`. To include Ben you would join on `:order-item/price-at-purchase` instead:

```datalog
(query [:find ?customer-name (sum ?price)
        :with ?item
        :where [?customer :customer/name ?customer-name]
               [?order :order/customer ?customer]
               [?item :order-item/order ?order]
               [?item :order-item/price-at-purchase ?price]])
```

```
?customer-name	(sum ?price)
----------------------------------------
"Ben"	1299

1 result(s) found.
```

---

## Understanding `:with` — controlling grouping granularity

`:with` adds extra variables to the grouping key without including them in the output columns. It does not change what is aggregated — it changes *how finely* rows are grouped before the aggregate is applied.

Consider the total spend query for Alice's order 2, which contains two USB-C cables at $19 each:

```datalog
; Groups by (?name, ?price) — both cables share the same name and price,
; so they collapse into one group. sum adds both contributions: 19 + 19 = 38.
(query [:find ?name (sum ?price)
        :where [?item :order-item/order :alice-order-2]
               [?item :order-item/product ?p]
               [?p :product/name ?name]
               [?item :order-item/price ?price]])
```

```
?name	(sum ?price)
----------------------------------------
"USB-C Cable 2m"	38

1 result(s) found.
```

One row, total $38 — both cables are summed into the single `("USB-C Cable 2m", 19)` group.

Now add `:with ?item`:

```datalog
; Groups by (?name, ?price, ?item) — each item entity is its own group.
; sum within each group = 19 per group.
(query [:find ?name (sum ?price)
        :with ?item
        :where [?item :order-item/order :alice-order-2]
               [?item :order-item/product ?p]
               [?p :product/name ?name]
               [?item :order-item/price ?price]])
```

```
?name	(sum ?price)
----------------------------------------
"USB-C Cable 2m"	19
"USB-C Cable 2m"	19

2 result(s) found.
```

Two rows, $19 each — one per line-item entity. `?item` is the hidden key that prevents the two cables from merging. It does not appear in the output.

**When to use `:with`**: reach for it when you want per-entity contributions to remain visible in the output — for example, a per-order-item breakdown that retains individual rows even when the projected values are identical. For a clean aggregate total (like total spend per customer), the standard grouping by plain variables is the right tool.

---

## Cheapest product per category

`min` and `max` work over any group the same way `sum` does:

```datalog
(query [:find ?category-name (min ?price)
        :where [?p :product/price ?price]
               [?p :product/category ?cat]
               [?cat :category/name ?category-name]])
```

```
?category-name	(min ?price)
----------------------------------------
"Accessories"	19
"Electronics"	449
"Laptops"	699
"Mobile"	599
"Noise-Cancelling"	249

5 result(s) found.
```

One row per category. Within Laptops, the cheapest is BudgetBook 14 at $699 — LaptopPro 15 now costs $1,229 after the tx 17 price drop. Within Accessories, the cheapest is USB-C Cable 2m at $19.

Note: `monitor-27` is assigned `:cat-electronics` directly (it is not in a leaf category), so "Electronics" here means the monitor at $449.

### Count products per category

```datalog
(query [:find ?category-name (count ?p)
        :where [?p :product/category ?cat]
               [?cat :category/name ?category-name]])
```

```
?category-name	(count ?p)
----------------------------------------
"Accessories"	2
"Electronics"	1
"Laptops"	2
"Mobile"	2
"Noise-Cancelling"	1

5 result(s) found.
```

---

## Window functions

Unlike aggregates, which collapse many rows into one per group, window functions **annotate each row** with a computed value. The number of rows in the result is the same as the number of rows in the input — nothing is merged.

Window functions appear in `:find` using an `:over` clause that specifies the window:

```
(rank  :over (:partition-by ?var :order-by ?var :desc))
(sum   ?x    :over (:order-by ?var))
(row-number  :over (:order-by ?var))
```

- `:partition-by ?var` — restarts the window computation for each distinct value of `?var` (equivalent to SQL `PARTITION BY`)
- `:order-by ?var` — sorts rows within the window before computing; `:desc` reverses the order
- Omitting `:partition-by` applies the window over all rows

### Rank products by price within category

```datalog
(query [:find ?category-name ?product-name ?price
              (rank :over (:partition-by ?cat :order-by ?price :desc))
        :where [?p :product/name ?product-name]
               [?p :product/price ?price]
               [?p :product/category ?cat]
               [?cat :category/name ?category-name]])
```

```
?category-name	?product-name	?price	(rank :over ...)
--------------------------------------------------------------------------------
"Accessories"	"Compact Keyboard"	89	1
"Accessories"	"USB-C Cable 2m"	19	2
"Electronics"	"ClearView 27" Monitor"	449	1
"Laptops"	"LaptopPro 15"	1229	1
"Laptops"	"BudgetBook 14"	699	2
"Mobile"	"PhoneX 12"	799	1
"Mobile"	"PhoneX 11"	599	2
"Noise-Cancelling"	"NoiseCancel Pro"	249	1

8 result(s) found.
```

All 8 products are present — no rows were removed. Within each category, rank 1 is the most expensive (`:order-by ?price :desc`). LaptopPro 15 at $1,229 is rank 1 in Laptops (the price drop at tx 17 did not affect its relative rank); BudgetBook 14 at $699 is rank 2.

### Row number over all products by price ascending

```datalog
(query [:find ?product-name ?price (row-number :over (:order-by ?price))
        :where [?p :product/name ?product-name]
               [?p :product/price ?price]])
```

```
?product-name	?price	(row-number :over ...)
------------------------------------------------------------
"USB-C Cable 2m"	19	1
"Compact Keyboard"	89	2
"NoiseCancel Pro"	249	3
"ClearView 27" Monitor"	449	4
"PhoneX 11"	599	5
"BudgetBook 14"	699	6
"PhoneX 12"	799	7
"LaptopPro 15"	1229	8

8 result(s) found.
```

All 8 rows with sequential position numbers 1–8 in ascending price order. No `:partition-by` means a single global window.

### Running cumulative sum of prices

```datalog
(query [:find ?product-name ?price (sum ?price :over (:order-by ?price))
        :where [?p :product/name ?product-name]
               [?p :product/price ?price]])
```

```
?product-name	?price	(sum ?price :over ...)
------------------------------------------------------------
"USB-C Cable 2m"	19	19
"Compact Keyboard"	89	108
"NoiseCancel Pro"	249	357
"ClearView 27" Monitor"	449	806
"PhoneX 11"	599	1405
"BudgetBook 14"	699	2104
"PhoneX 12"	799	2903
"LaptopPro 15"	1229	4132

8 result(s) found.
```

The third column is the running total: USB-C Cable $19, then $19 + $89 = $108, then $108 + $249 = $357, and so on up to $4,132 for the full catalog. Every row is preserved; the cumulative value grows as the window advances.

---

## Key concepts

- **Scalar aggregates** (`count`, `count-distinct`, `sum`, `sum-distinct`, `min`, `max`) collapse rows — one output row per group. Plain variables in `:find` are grouping keys; aggregates compute over all rows in each group.
- **`:with ?var`** — adds `?var` to the grouping key without projecting it into the output. Use this when you want per-entity contributions to remain as separate rows even when their projected values are identical. For straightforward aggregate totals, the standard grouping by plain variables is usually sufficient.
- **Window functions** annotate rows without collapsing them — the result has the same number of rows as the input.
- **`:partition-by ?var`** restarts window computation for each distinct value of `?var`; omitting it creates a single global window over all rows.
- **`:order-by ?var`** sorts within the window; add `:desc` to reverse. The sort order determines the meaning of `rank`, `row-number`, and cumulative aggregates like `sum :over`.

---

← [Section 5: Negation](tutorial-05-negation) | → [Section 7: Expression clauses](tutorial-07-expressions)

Reference: [Aggregation](datalog-reference#aggregation), [Window functions](datalog-reference#window-functions)
