---
title: Tutorial 7: Expression clauses and predicates
nav: 7. Expressions
section: Tutorial
order: 7
---
← [Section 6: Aggregates](tutorial-06-aggregates) | → [Section 8: Prepared queries](tutorial-08-prepared-queries)

---

## Scenario

The Corestore team needs to answer questions that go beyond pattern matching: which products are affordable, what did an order line item actually cost, which deliveries arrived late, and which SKUs follow the expected naming convention? These questions call for **expression clauses** — inline computations and predicates that run inside a `:where` block after the pattern clauses have bound their variables.

Expression clauses come in two forms. A **filter** keeps only the bindings where its expression is truthy:

```
[(< ?price 500)]
```

A **binding** evaluates its expression and assigns the result to a new variable that can be used in later clauses or in `:find`:

```
[(* ?qty ?price) ?line-total]
```

Both forms share one hard rule: every `?variable` that appears inside the expression must already have been bound by an earlier `:where` clause. Minigraf checks this statically — using an unbound variable is a parse error, not a silent no-result.

---

## Data setup (tx 18 and tx 19)

**Cumulative state at the start of this section:** tx_count = 17.

Two short transacts add fields needed for the examples in this section.

```datalog
; tx 18: add unit price to Ben's laptop line item
; (:order-item/qty 1 was already asserted in tx 6 — asserting it again would
;  create a second fact with a different valid-from, causing duplicate query rows)
(transact [
  [:ben-order-1-item-1 :order-item/price 1299]
])

; tx 19: add integer epoch values to Clara's order for delay arithmetic
(transact [
  [:clara-order-1 :order/promise-epoch 20229]
  [:clara-order-1 :order/actual-epoch  20234]
])
```

`20229` and `20234` are days since the Unix epoch. May 20 2026 is day 20229; May 25 2026 is day 20234 — five days late.

After tx 19, `tx_count` = 19.

---

## Filter predicates — price range

The most common expression clause is a comparison filter. Place a single predicate expression inside double brackets and Minigraf will discard any binding row where it is false.

```datalog
(query [:find ?name ?price
        :where [?p :product/name ?name]
               [?p :product/price ?price]
               [(< ?price 500)]])
```

```
?name                    ?price
----------------------------------------
"ClearView 27" Monitor"  449
"NoiseCancel Pro"        249
"Compact Keyboard"       89
"USB-C Cable 2m"         19

4 result(s) found.
```

All six comparison operators work the same way. They can compare a variable against a literal or compare two variables against each other:

```datalog
[(< ?price 500)]          ; less than
[(>= ?price 100)]         ; greater than or equal
[(= ?status :placed)]     ; equality — works on keywords too
[(!= ?sku "LP-15")]       ; not equal
[(< ?low ?high)]          ; two-variable comparison
[(>= ?actual ?promise)]   ; both sides are bound variables
```

---

## Arithmetic bindings — computing order totals

A binding expression evaluates an arithmetic expression and assigns the result to a new variable. Write the expression first and the target variable second, both inside the outer brackets:

```datalog
(query [:find ?product-name ?qty ?price ?line-total
        :where [:ben-order-1-item-1 :order-item/product ?p]
               [?p :product/name ?product-name]
               [:ben-order-1-item-1 :order-item/qty ?qty]
               [:ben-order-1-item-1 :order-item/price ?price]
               [(* ?qty ?price) ?line-total]])
```

```
?product-name   ?qty  ?price  ?line-total
------------------------------------------
"LaptopPro 15"  1     1299    1299

1 result(s) found.
```

`[(* ?qty ?price) ?line-total]` evaluates `?qty × ?price` and binds the result to `?line-total`. Once bound, `?line-total` is a regular variable: it can appear in subsequent expression clauses (for example, as input to another arithmetic step) or in `:find` as shown here.

Arithmetic operators: `+` `-` `*` `/`. Integer division truncates toward zero. Division by zero silently drops the row rather than raising an error.

---

## Finding late deliveries

The delivery-delay query shows the two forms working together. The binding computes the delay; the filter discards on-time orders:

```datalog
(query [:find ?customer-name ?delay-days
        :where [?order :order/customer ?customer]
               [?customer :customer/name ?customer-name]
               [?order :order/promise-epoch ?promise]
               [?order :order/actual-epoch ?actual]
               [(- ?actual ?promise) ?delay-days]
               [(> ?delay-days 0)]])
```

```
?customer-name  ?delay-days
----------------------------
"Clara"         5

1 result(s) found.
```

`[(- ?actual ?promise) ?delay-days]` runs first and binds `?delay-days`. Then `[(> ?delay-days 0)]` can use it because it now appears in a later clause. Reversing the two expression clauses would be a parse error — see the next section.

---

## Safety rule — variable binding order

Minigraf validates that every variable referenced inside an expression is already bound at the point where the expression clause appears. This check runs during parsing, not at query time.

```datalog
; PARSE ERROR — ?delay-days is referenced before it is bound
(query [:find ?customer-name
        :where [?order :order/customer ?customer]
               [?customer :customer/name ?customer-name]
               [(> ?delay-days 0)]            ; ← ?delay-days not yet bound
               [?order :order/promise-epoch ?promise]
               [?order :order/actual-epoch ?actual]
               [(- ?actual ?promise) ?delay-days]])
```

Fix: move the binding clause before any clause that uses the variable it introduces.

```datalog
; Correct ordering
(query [:find ?customer-name
        :where [?order :order/customer ?customer]
               [?customer :customer/name ?customer-name]
               [?order :order/promise-epoch ?promise]
               [?order :order/actual-epoch ?actual]
               [(- ?actual ?promise) ?delay-days]  ; binds ?delay-days first
               [(> ?delay-days 0)]])               ; then filters on it
```

---

## String predicates

Four string predicates are available as filter clauses: `starts-with?`, `ends-with?`, `contains?`, and `matches?`. All four take a string variable as their first argument.

```datalog
; Products whose SKU starts with "LP"
(query [:find ?name ?sku
        :where [?p :product/name ?name]
               [?p :product/sku ?sku]
               [(starts-with? ?sku "LP")]])
```

```
?name           ?sku
-----------------------
"LaptopPro 15"  "LP-15"

1 result(s) found.
```

```datalog
; Products whose name contains "Phone"
(query [:find ?name
        :where [?p :product/name ?name]
               [(contains? ?name "Phone")]])
```

```
?name
----------
"PhoneX 11"
"PhoneX 12"

2 result(s) found.
```

`ends-with?` works symmetrically to `starts-with?`. `matches?` accepts a regular expression string and returns true when the value matches anywhere in the string; an invalid regex is a parse error.

---

## Type predicates

Type predicates filter bindings to rows where the value has a specific Minigraf type. They are useful when an attribute stores mixed-type values or when you want to assert that data is well-formed.

```datalog
; Confirm all product prices are integers
(query [:find ?name ?price
        :where [?p :product/name ?name]
               [?p :product/price ?price]
               [(integer? ?price)]])
```

```
?name                    ?price
----------------------------------------
"ClearView 27" Monitor"  449
"PhoneX 12"              799
"LaptopPro 15"           1229
"PhoneX 11"              599
"NoiseCancel Pro"        249
"USB-C Cable 2m"         19
"Compact Keyboard"       89
"BudgetBook 14"          699

8 result(s) found.
```

All eight products have integer prices, so all eight rows survive the filter.

```datalog
; Confirm customer emails are strings
(query [:find ?name ?email
        :where [?c :customer/name ?name]
               [?c :customer/email ?email]
               [(string? ?email)]])
```

```
?name    ?email
--------------------------------------------
"Alice"  "alice@example.com"
"Clara"  "clara@example.com"
"Ben"    "ben@example.com"

3 result(s) found.
```

Also available: `float?`, `boolean?`, `nil?`. Type predicates can be used as filters (as shown) or as bindings — `[(integer? ?price) ?flag]` binds `true` or `false` to `?flag`, which is useful when you want the type check result as a column rather than a row selector.

---

## Arithmetic in aggregation

Expression bindings compose with aggregates. The pattern is: bind the computed value to a variable inside `:where`, then aggregate that variable in `:find`.

```datalog
; Total order value using qty × price, grouped by customer
(query [:find ?customer-name (sum ?line-total)
        :where [?customer :customer/name ?customer-name]
               [?order :order/customer ?customer]
               [?item :order-item/order ?order]
               [?item :order-item/price ?price]
               [?item :order-item/qty ?qty]
               [(* ?price ?qty) ?line-total]])
```

```
?customer-name  (sum ?line-total)
----------------------------------
"Alice"         837
"Ben"           1299

2 result(s) found.
```

Alice's total is 837: $799 for the PhoneX 12 (alice-order-1-item-1) plus $19 + $19 for the two USB-C cables in alice-order-2. Ben's total is $1,299 for the laptop. Clara's items are excluded because none of her order items carry an `:order-item/qty` fact — the join silently produces no rows for her.

Note that this query omits `:with`. Without `:with`, `?customer-name` is the sole grouping key and `(sum)` accumulates all matching line-totals into one row per customer. Adding `:with ?item` would make each item its own group, returning one row per item with each individual total rather than the per-customer grand total. See [Section 6](tutorial-06-aggregates) for a detailed explanation of `:with` and grouping granularity.

---

## Key concepts

- **Filter**: `[(expr)]` — keeps bindings where the expression is truthy; introduces no new variables.
- **Binding**: `[(expr) ?var]` — evaluates the expression and binds the result to `?var`; `?var` is available to all later clauses.
- **Variable binding order**: every `?variable` referenced inside an expression must be bound by an earlier `:where` clause — this is checked at parse time.
- **Arithmetic operators**: `+` `-` `*` `/` — integer division truncates; divide-by-zero drops the row silently.
- **Comparison operators**: `<` `>` `<=` `>=` `=` `!=` — work on integers, floats, and strings; `=` and `!=` also work on keywords.
- **String predicates**: `starts-with?` `ends-with?` `contains?` `matches?` — take a string variable and a literal argument.
- **Type predicates**: `string?` `integer?` `float?` `boolean?` `nil?` — usable as filters or as bindings that return `true`/`false`.
- **Composing with aggregates**: bind the computed value in `:where`, then aggregate the variable in `:find`.

---

← [Section 6: Aggregates](tutorial-06-aggregates) | → [Section 8: Prepared queries](tutorial-08-prepared-queries)

Reference: [Arithmetic & predicate expressions](datalog-reference#arithmetic--predicate-expressions)
