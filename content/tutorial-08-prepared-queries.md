---
title: Tutorial 8: Prepared queries
nav: 8. Prepared queries
section: Tutorial
order: 8
---
← [Section 7: Expression clauses](tutorial-07-expressions) | → [Section 9: Disjunction](tutorial-09-disjunction)

---

## Scenario

Corestore's backend runs the same queries on every customer page load, every product lookup, and every delivery status check. Parsing the same query string on every API call wastes CPU — and in a hot path that fires hundreds of times per second, that waste compounds quickly. **Prepared queries** move parsing to startup time; bind slots substitute the variable parts at execute time.

---

## Why prepare?

Parsing a Datalog query involves tokenising the input, building an AST and checking safety constraints (every variable in `:find` must be bound in `:where`). When the query shape never changes between calls, doing that work on every invocation is unnecessary overhead. Prepared queries pay the parse cost once at startup; each subsequent `execute()` substitutes the bind values, plans the query with them (index selection and join reordering) and runs it. Planning with the actual values lets the planner use them.

---

## Bind slot syntax

A bind slot is any `$identifier` token in the query string. It marks a position that will be filled with a concrete value at execute time.

```datalog
; String query — re-parsed and re-planned on every call
(query [:find ?price
        :where [:laptop-pro :product/price ?price]])

; Equivalent prepared query template
(query [:find ?price
        :where [$product :product/price ?price]])
;              ↑ bind slot — filled at execute time
```

The permitted `BindValue` variant depends on where the slot appears:

<!-- @until v3.0.0 -->
| Position | Permitted `BindValue` variants |
|---|---|
| Entity in pattern `[$entity :attr ?v]` | `BindValue::Entity(Uuid)` |
| Value in pattern `[?e :attr $val]` | `BindValue::Val(Value)` |
| `:as-of $tx` | `BindValue::TxCount(u64)`, `BindValue::Timestamp(i64)` |
| `:valid-at $date` | `BindValue::Timestamp(i64)`, `BindValue::AnyValidTime` |
<!-- @end -->
<!-- @since v3.0.0 -->
| Position | Permitted `BindValue` variants |
|---|---|
| Entity in pattern `[$entity :attr ?v]` | `BindValue::Entity(Uuid)`, `BindValue::Val(Value::Ref(Uuid))` |
| Value in pattern `[?e :attr $val]`, expression, rule argument | `BindValue::Val(Value)`, `BindValue::Entity(Uuid)` |
| `:as-of $tx` | `BindValue::TxCount(u64)`, `BindValue::Timestamp(i64)` |
| `:valid-at $date` | `BindValue::Timestamp(i64)`, `BindValue::AnyValidTime` |
<!-- @end -->

---

## Example 1: Product price lookup

Prepare once at startup; execute with a different product UUID on every request.

```rust
use minigraf::{Minigraf, OpenOptions, BindValue};

let db = OpenOptions::new().open()?;
// ... load product data ...

// Prepare once — parse and plan cost paid here
let price_query = db.prepare(
    "(query [:find ?price
             :where [$product :product/price ?price]])"
)?;

// Execute with different products — no re-parsing
let laptop_price = price_query.execute(&[
    ("product", BindValue::Entity(laptop_pro_id)),
])?;
// → [[Value::Integer(1229)]]

let phone_price = price_query.execute(&[
    ("product", BindValue::Entity(phone_x_id)),
])?;
// → [[Value::Integer(799)]]
```

> **Note on UUIDs.** `laptop_pro_id` is the UUID that Minigraf assigned to the `:laptop-pro` keyword when it was first transacted. In the REPL, keywords resolve to stable per-session UUIDs. In production code, capture the UUID from the first transact response and store it for later use as a bind value.

---

## Example 2: Order status with an `:as-of` bind slot

Parameterise the transaction count to replay history for any order at any past tx.

```rust
use minigraf::{Minigraf, OpenOptions, BindValue};

// Prepare: what was an order's status at a given tx?
let status_query = db.prepare(
    "(query [:find ?status
             :as-of $tx
             :where [$order :order/status ?status]])"
)?;

// At tx 6 (when Ben's order was placed)
let at_placement = status_query.execute(&[
    ("tx",    BindValue::TxCount(6)),
    ("order", BindValue::Entity(ben_order_id)),
])?;
// → [[Value::Keyword(":placed")]]

// At tx 14 (after the delivery update)
let at_delivery = status_query.execute(&[
    ("tx",    BindValue::TxCount(14)),
    ("order", BindValue::Entity(ben_order_id)),
])?;
// → [[Value::Keyword(":delivered")]]
```

`:as-of $tx` accepts either `BindValue::TxCount(u64)` (a monotonic counter) or `BindValue::Timestamp(i64)` (a Unix millisecond timestamp). Use `TxCount` when you tracked the tx number; use `Timestamp` when you only have a wall-clock instant.

---

## Example 3: Sale price at a given valid-at date

Parameterise the valid-time dimension to look up business-time facts at any point.

```rust
use minigraf::{Minigraf, OpenOptions, BindValue};

let sale_query = db.prepare(
    "(query [:find ?price
             :valid-at $date
             :where [$product :product/sale-price ?price]])"
)?;

// What was the sale price on 2026-01-15?
let jan_price = sale_query.execute(&[
    ("product", BindValue::Entity(laptop_pro_id)),
    ("date",    BindValue::Timestamp(1736899200000)), // 2026-01-15 00:00 UTC in ms
])?;
// → [[Value::Integer(1049)]]   (corrected winter sale price)

// Lift the valid-time filter entirely — return all sale-price facts regardless of valid window
let all_prices = sale_query.execute(&[
    ("product", BindValue::Entity(laptop_pro_id)),
    ("date",    BindValue::AnyValidTime),
])?;
// → [[Value::Integer(1049)] [Value::Integer(1149)]]   (both sale windows)
```

`BindValue::AnyValidTime` is a special sentinel that tells the executor to skip the valid-time filter entirely — equivalent to querying without a `:valid-at` clause. This makes it easy to write a single prepared query that handles both "at a specific date" and "show me everything" without duplicating the query template.

---

## What cannot be parameterised — attribute position

The attribute position in a triple pattern is **not** a valid bind slot:

```datalog
; NOT allowed — attribute position cannot be a bind slot:
(query [:find ?value
        :where [?entity $attr ?value]])
;                        ↑ rejected by prepare()
```
<!-- @since v3.0.0 -->
`prepare()` rejects it with `API-023`. A missing bind value at execute time is `API-022`, and a bind value of the wrong kind for its slot is `API-024`.
<!-- @end -->

The reason is architectural: Minigraf's query optimizer selects which index to use — EAVT, AEVT, AVET, or VAET — based on which positions of a pattern are known, and the attribute is the primary discriminator for that decision. A query whose attribute could change would need a different kind of plan for every value, so the attribute is fixed when the query is prepared.

When you genuinely need to query across different attributes, write separate prepared queries — one per attribute — and call whichever one matches the lookup you need.

---

## Live fact store behavior

`PreparedQuery` holds `Arc` clones of the live fact store. Each `execute()` sees the **current** state of the database — including facts transacted after `prepare()` was called. This is by design: you prepare once at application startup and the query stays live as data changes without any manual cache invalidation.

```rust
use minigraf::{Minigraf, OpenOptions, BindValue};

let db = OpenOptions::new().open()?;
// ... load initial data ...

// Prepare before the price update
let q = db.prepare(
    "(query [:find ?price
             :where [:laptop-pro :product/price ?price]])"
)?;

// Transact a new price after prepare()
db.execute("(transact [[:laptop-pro :product/price 1199]])")?;

let result = q.execute(&[])?;
// → [[Value::Integer(1199)]]  — sees the new price, even though prepared before it
```

---

<!-- @since v3.0.0 -->
## In the browser

`BrowserDb.prepare()` gives the browser the same prepared queries. Bindings are a plain object of slot name (without the `$`) to value:

```js
const pq = db.prepare(`(query [:find ?name
                               :where [?p :product/price ?price]
                                      [(< ?price $max)]
                                      [?p :product/name ?name]])`);
const cheap = JSON.parse(pq.execute({ max: 500 })).results;
const mid = JSON.parse(pq.execute({ max: 1000 })).results;
```

A boolean, a string, `null`, a `bigint` and a `number` bind as values; a `number` is an integer when `Number.isInteger` accepts it and a float otherwise. The other kinds use a `{ type, value }` object: `{ type: 'float', value: 2 }` for a float with no fractional part, `'keyword'`, `'ref'`, `{ type: 'entity', value: uuid }` for an entity slot, `{ type: 'txCount', value: 50 }` or `{ type: 'timestamp', value: Date.now() }` for `:as-of`, and `{ type: 'anyValidTime' }` for `:valid-at`. `pq.query(bindings)` returns a cursor, as `db.query()` does.

A set binds as `{ type: 'set', value: [...] }` or a JS `Set`, for the [set predicates](datalog-reference#sets) or a set-valued attribute. Its elements are plain or typed values, so a keyword element is written typed: `pq.execute({ roles: new Set([{ type: 'keyword', value: ':role/admin' }]) })`. An element in an entity, tx count, timestamp or any-valid-time form is `API-017`, a set inside a set `API-021`, and a plain array is not a set.

## In the other bindings

Every binding has the same prepared queries: `prepare(text)` once, then `execute(bindings)` for the JSON result or `query(bindings)` for a cursor. Plain host values bind as values (booleans, integers, floats, strings, null), a UUID type binds an entity, and a typed bind value covers the other kinds.

```python
pq = db.prepare("(query [:find ?name :where [?p :product/price ?price] [(< ?price $max)] [?p :product/name ?name]])")
cheap = json.loads(pq.execute(max=500))["results"]
mid = json.loads(pq.execute({"max": 1000}))["results"]
# minigraf.Entity(uuid), minigraf.TxCount(n), minigraf.Timestamp(ms), minigraf.ANY_VALID_TIME
```

```kotlin
val cheap = pq.execute("max" to 500)            // Kotlin (JVM and Android)
```

```java
String cheap = Minigraf.execute(pq, Map.of("max", 500));   // Java
```

```swift
let cheap = try pq.execute(["max": 500])        // Swift
```

```js
const cheap = JSON.parse(pq.execute({ max: 500 })).results   // Node.js
```

```c
char *cheap = minigraf_prepared_execute(pq, "{\"max\": 500}");   /* C: bindings as JSON */
```

A missing slot is `API-022`, a slot in an attribute position `API-023`, and a value of the wrong kind for its slot `API-024`. In Python, Java, Android and Swift these raise `MiniGrafError.Api`. In Node.js and C, closing the database releases its prepared queries, and one used afterwards fails with `API-018`. Each binding's README lists its plain and typed forms.
<!-- @end -->

## REPL equivalent

The REPL does not support bind slots — every query uses literal values. Prepared queries are an API feature for production applications where parse overhead matters and query shapes are stable. When exploring the database interactively or testing query logic, write queries with literal values in the REPL; port them to prepared queries when integrating into application code.

---

## Key concepts

- **`$slot`** — named bind slot; marks a position to be substituted at execute time.
- **`db.prepare(str)`** — parses and validates the query once; returns a `PreparedQuery`.
- **`pq.execute(&[("slot", BindValue::...)])`** — substitutes bind values, plans the query and runs it.
- **Attribute position is not parameterisable** — index selection depends on knowing the attribute.
- **Each `execute()` sees live state** — `PreparedQuery` holds `Arc` references to the fact store; no staleness.
- **`:as-of $tx`** accepts `TxCount(u64)` or `Timestamp(i64)`.
- **`:valid-at $date`** accepts `Timestamp(i64)` or `AnyValidTime` (lifts the filter entirely).

---

← [Section 7: Expression clauses](tutorial-07-expressions) | → [Section 9: Disjunction](tutorial-09-disjunction)

Reference: [Prepared statements](datalog-reference#prepared-statements-rust-api)
