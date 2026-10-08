---
title: Tutorial 10: User-Defined Functions
nav: 10. User-defined functions
section: Tutorial
order: 10
---
← [Section 9: Disjunction](tutorial-09-disjunction) | → [Section 11: Marketplace](tutorial-11-marketplace)

---

## Scenario

Corestore needs two domain-specific computations that no built-in function covers. The promotions team wants to filter promo codes by a Corestore-specific validity rule: a valid code must start with `"CORESTORE-"` and be at least 15 characters long. The operations team wants a per-customer delivery score — the fraction of orders delivered on time — computed as a single float.

Both requirements can be met by registering **user-defined functions** (UDFs) via the Rust API, then using them in ordinary Datalog queries. UDFs integrate seamlessly with the rest of the query language: predicate UDFs appear in `:where` filter expressions, and aggregate UDFs appear in `:find` exactly like built-in aggregates.

UDFs cannot be registered from the REPL; they are a library API feature. The complete runnable example for this section is at `examples/tutorial_udfs.rs`.

---

## Data setup

The section uses a fresh in-memory database (no cumulative state needed from prior sections). Load the seed data:

```datalog
; Promo codes — one valid Corestore code, two that fail the rule
(transact [
  [:promo-1 :promo/code "CORESTORE-SUMMER2026"]
  [:promo-2 :promo/code "SAVE10"]
  [:promo-3 :promo/code "PARTNER-EXCLUSIVE"]
])

; Customers + orders with a pre-computed on-time flag (1 = on time, 0 = late)
;   Alice: order-a (on time), order-b (late)  → expected score 0.5
;   Ben:   order-c (on time)                  → expected score 1.0
(transact [
  [:alice :customer/name "Alice"]
  [:ben   :customer/name "Ben"]
  [:order-a :order/customer :alice]
  [:order-a :order/on-time-flag 1]
  [:order-b :order/customer :alice]
  [:order-b :order/on-time-flag 0]
  [:order-c :order/customer :ben]
  [:order-c :order/on-time-flag 1]
])
```

---

## Predicate UDFs

A **predicate UDF** is a single-argument Rust closure that returns a `bool`. Register it with `Minigraf::register_predicate`:

*All Rust snippets in this section are written in the context of a `fn main() -> anyhow::Result<()>` function. The `?` operator requires a `Result`-returning context.*

```rust
pub fn register_predicate(
    &self,
    name: &str,
    f: impl Fn(&Value) -> bool + Send + Sync + 'static,
) -> Result<()>
```

### Registering `valid-promo?`

```rust
use minigraf::{Minigraf, Value};

let db = Minigraf::in_memory()?;

db.register_predicate(
    "valid-promo?",
    |v: &Value| {
        if let Value::String(s) = v {
            s.starts_with("CORESTORE-") && s.len() >= 15
        } else {
            false
        }
    },
)?;
```

The name `valid-promo?` follows the Lisp `?`-suffix convention for predicates. The convention is not enforced — any name is accepted — but it signals intent to readers.

### Using a predicate UDF in `:where`

A registered predicate appears as a single-argument expression inside a vector:

```datalog
(query [:find ?code
        :where [?p :promo/code ?code]
               [(valid-promo? ?code)]])
```

```
?code
--------------------
"CORESTORE-SUMMER2026"

1 result(s) found.
```

The expression `[(valid-promo? ?code)]` is a filter: for each binding of `?code`, the engine calls the registered closure. Only bindings where the closure returns `true` survive. `"SAVE10"` (too short) and `"PARTNER-EXCLUSIVE"` (wrong prefix) are filtered out.

---

## Aggregate UDFs

An **aggregate UDF** is a typed state machine with three stages: initialise, step, and finalise. Register it with `Minigraf::register_aggregate`:

```rust
pub fn register_aggregate<Acc>(
    &self,
    name: &str,
    init:     impl Fn() -> Acc               + Send + Sync + 'static,
    step:     impl Fn(&mut Acc, &Value)       + Send + Sync + 'static,
    finalise: impl Fn(&Acc, usize) -> Value   + Send + Sync + 'static,
) -> Result<()>
where
    Acc: Any + Send + 'static,
```

The type parameter `Acc` is your accumulator type. The engine creates one accumulator per group (by calling `init`), calls `step` for each row in the group, and then calls `finalise` to produce the output value. The `usize` argument to `finalise` is the number of rows processed in the group.

### Registering `delivery-score`

```rust
use minigraf::{Minigraf, Value};

let db = Minigraf::in_memory()?;

// Accumulator: (on_time_count, total_count)
db.register_aggregate(
    "delivery-score",
    || (0i64, 0i64),
    |state: &mut (i64, i64), val: &Value| {
        if let Value::Integer(flag) = val {
            state.1 += 1;             // always increment total
            if *flag == 1 {
                state.0 += 1;         // increment on-time count
            }
        }
    },
    |state: &(i64, i64), _n: usize| {
        if state.1 == 0 {
            Value::Null
        } else {
            Value::Float(state.0 as f64 / state.1 as f64)
        }
    },
)?;
```

The accumulator is a plain Rust tuple — no trait objects or `Any` downcasting needed in the closures, because `register_aggregate` erases the type internally.

---

## Using a UDF aggregate in `:find`

Use a registered aggregate in `:find` exactly like any built-in aggregate:

```datalog
(query [:find ?name (delivery-score ?flag)
        :where [?customer :customer/name ?name]
               [?order :order/customer ?customer]
               [?order :order/on-time-flag ?flag]])
```

```
?name     (delivery-score ?flag)
------------------------------------
"Alice"   0.5
"Ben"     1.0

2 result(s) found.
```

The query groups by `?name`. For Alice, the group contains two rows with flags `1` and `0` → score = 1/2 = 0.5. For Ben, one row with flag `1` → score = 1/1 = 1.0.

---

## UDF aggregates in window clauses

A custom aggregate also works in an `:over` window clause, allowing per-row annotation without collapsing groups. The `:over` clause requires both `:partition-by` and `:order-by`:

```datalog
(query [:find ?order (delivery-score ?flag :over (:partition-by ?customer :order-by ?order))
        :where [?customer :customer/name ?name]
               [?order :order/customer ?customer]
               [?order :order/on-time-flag ?flag]])
```

```
?order     (delivery-score ?flag :over ...)
--------------------------------------------
<uuid>      1.0
<uuid>      0.5
<uuid>      1.0

3 result(s) found.
```

This annotates each order row with a running delivery score within its customer partition. The `:partition-by ?customer` groups rows by customer entity; `:order-by ?order` determines the accumulation order within each partition. Each row receives the fraction computed so far over orders processed up to and including that row for the same customer. (Entity identifiers are UUID-based and vary between runs; the score values are deterministic.)

Note: the `:order-by` key is required in every `:over` clause — omitting it results in a runtime error.

Window UDFs follow the same `:over` syntax as built-in window functions — see [Section 6](tutorial-06-aggregates) for the full window function reference.

---

## Runtime resolution

UDF names in `:find` aggregates, `:over` window clauses, and `[(name? ?var)]` filter expressions are **not validated at parse time**. The parser records the name as an opaque identifier and defers the lookup to execution. If the name is not registered when the query runs, the executor returns an error:

<!-- @until v3.0.0 -->
```
Error: [INT-029] unknown aggregate function: 'delivery-score'
```
<!-- @end -->
<!-- @since v3.0.0 -->
```
Error: [QRY-010] unknown aggregate function: 'delivery-score'
```

Aggregate and window function names are looked up when the query starts, so the error comes even when no row matches.
<!-- @end -->

This means you can parse and store a query string before calling `register_predicate` or `register_aggregate`. The query will execute successfully once the UDF is registered.

```rust
// Parse a query that references a UDF not yet registered
let prepared = db.prepare(
    r#"(query [:find ?code
               :where [?p :promo/code ?code]
                      [(valid-promo? ?code)]])"#,
)?;

// ... later, or in a different initialisation step:
db.register_predicate("valid-promo?", |v| {
    matches!(v, Value::String(s) if s.starts_with("CORESTORE-") && s.len() >= 15)
})?;

// Now execution succeeds
let results = prepared.execute(&[])?;
```

---

## Running the example

The complete annotated example is at `examples/tutorial_udfs.rs`. Run it with:

```bash
cargo run --example tutorial_udfs
```

Expected output:

```
=== Predicate UDF: valid-promo? ===

Query: find promo codes that satisfy valid-promo?

  (query [:find ?code
          :where [?p :promo/code ?code]
                 [(valid-promo? ?code)]])

?code
--------------------
"CORESTORE-SUMMER2026"

1 result(s) found.

=== Aggregate UDF: delivery-score ===

Query: on-time delivery score per customer

  (query [:find ?name (delivery-score ?flag)
          :where [?customer :customer/name ?name]
                 [?order :order/customer ?customer]
                 [?order :order/on-time-flag ?flag]])

?name     (delivery-score ?flag)
------------------------------------
"Alice"   0.5
"Ben"     1.0

2 result(s) found.

=== UDF aggregate in a window clause ===

Query: annotate each order with its customer delivery score

?order     (delivery-score ?flag :over ...)
--------------------------------------------
<uuid>      1.0
<uuid>      0.5
<uuid>      1.0

3 result(s) found.
```

Order entity IDs are UUID-based and vary between runs. The three score values (1.0, 0.5, 1.0) are deterministic: Alice's two orders produce running scores within her partition, and Ben's single order scores 1.0.

---

## Key concepts

| Concept | Meaning |
|---|---|
| `register_predicate(name, f)` | Register a single-argument `&Value -> bool` filter. Use in `:where` as `[(name? ?var)]`. |
| `register_aggregate(name, init, step, finalise)` | Register a typed aggregate state machine. Use in `:find` as `(name ?var)` or in `:over` window clauses. |
| UDF aggregate in `:over` window | `(name ?var :over (:partition-by ?p :order-by ?x))` — annotates each row with the UDF result computed over its partition, without collapsing rows. `:order-by` is required. |
| Accumulator type `Acc` | Any `Send + 'static` type. Closures receive typed `&mut Acc` / `&Acc` — no manual downcasting needed. |
| `finalise(_n: usize)` | The `usize` argument is the row count for the group. Useful for averages or thresholds. |
| Runtime resolution | UDF names are resolved at execution time, not at parse time. A missing UDF name produces an execution error, not a parse error. |
| `?` suffix convention | Not enforced, but predicate names should end with `?` by convention (`valid-promo?`, `email?`, `active?`). |

---

← [Section 9: Disjunction](tutorial-09-disjunction) | → [Section 11: Marketplace](tutorial-11-marketplace)

Reference: [User-defined functions](datalog-reference#user-defined-functions)
