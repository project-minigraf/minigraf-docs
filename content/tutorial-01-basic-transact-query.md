---
title: Tutorial 1: Basic Transact and Query
nav: 1. Basic transact + query
section: Tutorial
order: 1
---
Alice has been browsing Corestore and decides to buy a PhoneX 12. She adds a USB-C cable to her order while checking out, then thinks better of it and removes it before confirming. This tutorial walks through asserting and retracting facts, and querying the results.

## The data model

Every piece of data in Minigraf is an **EAV triple**: an entity, an attribute, and a value. The entity is a keyword that names the thing, the attribute is a namespaced keyword describing the property, and the value is a string, number, boolean, or a reference to another entity.

```datalog
; A fact is a triple: [entity attribute value]
[:laptop-pro :product/name "LaptopPro 15"]
[:laptop-pro :product/price 1299]
[:laptop-pro :product/category :cat-laptops]   ; value is a reference to another entity
```

There are no schemas or table definitions — facts can be added freely, and entities can carry any combination of attributes.

## Querying the catalog

With the base data loaded, query all products and their prices:

```datalog
; List all products with their prices
(query [:find ?name ?price
        :where [?p :product/name ?name]
               [?p :product/price ?price]])
```

Expected output (8 rows, order may vary):

```
?name           ?price
----------------------------------------
"BudgetBook 14"       699
"LaptopPro 15"        1299
"PhoneX 12"           799
"USB-C Cable 2m"      19
"PhoneX 11"           599
"NoiseCancel Pro"     249
"ClearView 27" Monitor"  449
"Compact Keyboard"    89
```

**Variables** start with `?` and act as placeholders. The engine finds every fact matching each clause and unifies the variables across clauses. Here, `?p` appears in both clauses, so only entities that have both a `:product/name` and a `:product/price` will be included. `?name` and `?price` are projected into the result.

## Alice places her order (tx 4)

```datalog
; tx 4: Alice's first order
(transact [
  [:alice-order-1        :order/customer :alice]
  [:alice-order-1        :order/status :placed]
  [:alice-order-1-item-1 :order-item/order :alice-order-1]
  [:alice-order-1-item-1 :order-item/product :phone-x]
  [:alice-order-1-item-1 :order-item/qty 1]
  [:alice-order-1-item-1 :order-item/price 799]
  [:alice-order-1-item-2 :order-item/order :alice-order-1]
  [:alice-order-1-item-2 :order-item/product :usb-cable]
  [:alice-order-1-item-2 :order-item/qty 1]
  [:alice-order-1-item-2 :order-item/price 19]
])
```

All ten facts are asserted in a single atomic transaction. The order is represented as an entity (`:alice-order-1`) with two line-item entities (`:alice-order-1-item-1`, `:alice-order-1-item-2`). Each item references the order entity and the product entity it relates to.

## What's in Alice's order?

```datalog
(query [:find ?product-name ?qty ?price
        :where [?item :order-item/order :alice-order-1]
               [?item :order-item/product ?p]
               [?p :product/name ?product-name]
               [?item :order-item/qty ?qty]
               [?item :order-item/price ?price]])
```

Expected:

```
"PhoneX 12"      1   799
"USB-C Cable 2m" 1   19
```

The variable `?item` appears in multiple clauses. The engine constrains all those clauses to the same entity — only rows where the same `?item` satisfies every clause are included. This is how **multi-clause joins** work: shared variables act as join keys without any explicit `JOIN` syntax.

## Alice removes the USB cable (tx 5)

```datalog
; tx 5: Alice cancels the USB cable
(retract [
  [:alice-order-1-item-2 :order-item/order :alice-order-1]
  [:alice-order-1-item-2 :order-item/product :usb-cable]
  [:alice-order-1-item-2 :order-item/qty 1]
  [:alice-order-1-item-2 :order-item/price 19]
])
```

`retract` marks each listed fact as `asserted = false`. The original facts are not deleted — they remain in history and are visible via time-travel queries (covered in [Section 2: `:as-of`](tutorial-02-as-of) and [Section 3: `:valid-at`](tutorial-03-valid-at)). The current view of the database shows only net-asserted facts, so the cable line item disappears from queries.

**Visualize:** [see tx 5 in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#sample=corestore-tutorial&tx=5&e=:alice-order-1-item-2) The retracted facts show in red. Press ← to see the order before the retraction.

## What's in Alice's order now?

Re-run the same query from the previous section:

```datalog
(query [:find ?product-name ?qty ?price
        :where [?item :order-item/order :alice-order-1]
               [?item :order-item/product ?p]
               [?p :product/name ?product-name]
               [?item :order-item/qty ?qty]
               [?item :order-item/price ?price]])
```

Expected:

```
"PhoneX 12"  1  799
```

The USB cable no longer appears because its facts are retracted and excluded from the current view.

## All orders and their customers

```datalog
(query [:find ?customer-name ?status
        :where [?order :order/customer ?customer]
               [?customer :customer/name ?customer-name]
               [?order :order/status ?status]])
```

Expected:

```
"Alice"  :placed
```

This query joins across three entity types: an order, the customer it references, and the customer's name attribute.

## Key concepts

- A **fact** is an EAV triple `[entity attribute value]`
- `transact` asserts facts atomically; `retract` marks them as not-asserted
- **Variables** (`?name`) unify across clauses — the same variable name means the same value
- **Multi-clause joins** constrain results to matching combinations — no explicit join syntax needed

---

← [Setup](tutorial-setup) | → [Section 2: `:as-of`](tutorial-02-as-of)

Reference: [transact](datalog-reference#transact), [retract](datalog-reference#retract), [query](datalog-reference#query)
