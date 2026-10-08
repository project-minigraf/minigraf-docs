---
title: Datalog Reference
nav: Datalog reference
section: Reference
order: 10
---
Minigraf uses a Datalog dialect with EDN (Extensible Data Notation) syntax.
The machine-checkable version of this grammar is `tests/grammar/grammar.pest`.

---

## Formal Grammar (EBNF)

The following EBNF specifies the **structural** syntax accepted by the parser.
Semantic constraints (safety checks, binding rules, compatibility rules) are documented
separately in the [Semantic Constraints](#semantic-constraints) section below.

```ebnf
(* ── Top level ────────────────────────────────────────────────────────── *)
command ::= transact-cmd | retract-cmd | query-cmd | rule-cmd

(* ── Transact / Retract ───────────────────────────────────────────────── *)
transact-cmd   ::= "(" "transact" (valid-time-map fact-vector | fact-vector) ")"
retract-cmd    ::= "(" "retract" fact-vector ")"
fact-vector    ::= "[" fact* "]"
fact           ::= "[" edn-value edn-value edn-value valid-time-map? "]"
valid-time-map ::= "{" (":valid-from" string | ":valid-to" string)* "}"

(* ── Query ────────────────────────────────────────────────────────────── *)
query-cmd    ::= "(" "query" query-vector ")"
query-vector ::= "[" query-section+ "]"
query-section ::=
    find-section | where-section | as-of-section |
    valid-at-section | any-valid-time-section | with-section |
    max-derived-facts-section | max-results-section

max-derived-facts-section ::= ":max-derived-facts" integer
max-results-section       ::= ":max-results" integer

find-section           ::= ":find" find-spec+
where-section          ::= ":where" where-clause+
as-of-section          ::= ":as-of" (integer | string | bind-slot)
valid-at-section       ::= ":valid-at" (string | ":any-valid-time" | bind-slot)
any-valid-time-section ::= ":any-valid-time"
with-section           ::= ":with" variable+

(* ── Find specs ───────────────────────────────────────────────────────── *)
find-spec      ::= variable | aggregate-expr | window-expr
aggregate-expr ::= "(" symbol variable ")"
window-expr    ::= "(" symbol (variable ":over" | ":over") over-clause ")"
over-clause    ::= "(" over-option* ")"
over-option    ::= (":partition-by" | ":order-by") variable | ":desc" | ":asc"

(* ── Where clauses ────────────────────────────────────────────────────── *)
where-clause ::=
    pattern-clause | expr-clause | not-clause | not-join-clause |
    or-clause | or-join-clause | rule-invocation

pattern-clause  ::= "[" edn-value edn-value edn-value "]"
expr-clause     ::= "[" expr variable? "]"
not-clause      ::= "(" "not" where-clause+ ")"
not-join-clause ::= "(" "not-join" join-vars where-clause+ ")"
or-clause       ::= "(" "or" or-branch+ ")"
or-join-clause  ::= "(" "or-join" join-vars or-branch+ ")"
join-vars       ::= "[" variable* "]"
or-branch       ::= and-branch | where-clause
and-branch      ::= "(" "and" where-clause+ ")"
rule-invocation ::= "(" symbol edn-value* ")"

(* ── Expressions ──────────────────────────────────────────────────────── *)
expr        ::= "(" (unary-op | symbol) expr-arg ")"
              | "(" binary-op expr-arg expr-arg ")"
unary-op    ::= "string?" | "integer?" | "float?" | "boolean?" | "nil?"
binary-op   ::= "<" | ">" | "<=" | ">=" | "=" | "!=" | "+" | "-" | "*" | "/"
              | "starts-with?" | "ends-with?" | "contains?" | "matches?"
expr-arg    ::= expr | boolean | nil | integer | float | string
              | keyword | variable | bind-slot

(* ── Rule ─────────────────────────────────────────────────────────────── *)
rule-cmd    ::= "(" "rule" rule-vector ")"
rule-vector ::= "[" rule-head where-clause* "]"
rule-head   ::= "(" symbol variable* ")"

(* ── EDN values ───────────────────────────────────────────────────────── *)
edn-value ::= uuid | boolean | nil | float | integer | string | bind-slot
            | keyword | symbol | list | vector | map
list      ::= "(" edn-value* ")"
vector    ::= "[" edn-value* "]"
map       ::= "{" (edn-value edn-value)* "}"

(* ── Primitives ───────────────────────────────────────────────────────── *)
keyword   ::= ":" (letter | digit | "/" | "-" | "_" | "?")+
symbol    ::= (letter | "_") (letter | digit | "?" | "_" | "-" | "/")*
            | "-" (letter | digit | "?" | "_" | "-" | "/")+
variable  ::= "?" (letter | digit | "?" | "_" | "-" | "/")*
boolean   ::= "true" | "false"
nil       ::= "nil"
integer   ::= "-"? digit+
float     ::= "-"? digit+ "." digit*
string    ::= '"' str-char* '"'
str-char  ::= "\" ("n" | "t" | "r" | '"' | "\") | any-char-except-quote-backslash
uuid      ::= "#uuid" string
bind-slot ::= "$" (letter | digit | "_" | "-")+

(* letter = [a-zA-Z], digit = [0-9] *)
(* Whitespace (space, tab, newline, comma) is ignored between tokens. *)
```

---

## Semantic Constraints

The following constraints are enforced by the parser **above** the structural grammar layer.
A syntactically valid input may be rejected for violating one of these rules.

### Not-safety

Every variable referenced in a `(not ...)` body must be bound by an outer clause
appearing **before** the `not` in the same `:where` or rule body:

```datalog
;; INVALID — ?banned is not bound before the (not ...)
(query [:find ?name
        :where [?e :person/name ?name]
               (not [?banned :person/role :admin])])   ; ?banned unbound

;; VALID — ?e is bound by the outer pattern
(query [:find ?name
        :where [?e :person/name ?name]
               (not [?e :person/banned true])])
```

For `not-join`, every variable listed in the **join-vars vector** must be bound by an outer clause.
Variables that appear only in the `not-join` body but are **not** in the join-vars are
existentially quantified and do not need prior binding:

```datalog
;; VALID — ?e is the join var (bound above); ?dept is existential (body-only)
(not-join [?e]
  [?e :dept ?dept]
  [?dept :status :bad])
```

### Nested not

`(not ...)` cannot appear directly inside another `(not ...)` or `(not-join ...)`.
`(or ...)` and `(or-join ...)` cannot appear inside `(not ...)` or `(not-join ...)`.

### Expression variable binding

All variables referenced in an expression filter `[(expr)]` must be bound by an **earlier**
clause in the same `:where` or rule body (forward-pass check):

```datalog
;; INVALID — ?salary used before it is bound
(query [:find ?name
        :where [?e :person/name ?name]
               [(> ?salary 50000)]          ; ?salary not yet bound
               [?e :emp/salary ?salary]])

;; VALID — ?salary bound before the filter
(query [:find ?name
        :where [?e :person/name ?name]
               [?e :emp/salary ?salary]
               [(> ?salary 50000)]])
```

A binding expression `[(expr) ?out]` adds `?out` to the bound set for subsequent clauses.

<!-- @since v3.0.0 -->
### `:find` variables must be bound

Every variable in `:find` must be bound by the `:where` clauses; otherwise the query fails at parse time with `PRS-080`, naming the variable. This covers `execute()`, `query()` and `prepare()`, and a window function's argument, `:partition-by` and `:order-by` variables too. A variable counts as bound when it appears in a pattern, a rule call, an expression binding, an `or-join` join vector, or every branch of an `or`. One that appears only inside `not` / `not-join`, or a `?_` wildcard, does not.

```datalog
;; INVALID — ?nmae is a typo; nothing binds it (PRS-080)
(query [:find ?nmae :where [?e :person/name ?name]])
```

<!-- @end -->
<!-- @until v3.0.0 -->
A `:find` variable that no `:where` clause binds is not rejected: the query returns no results, so a typo in `:find` looks the same as "no matching data". From v3.0.0 this is an error (`PRS-080`).

<!-- @end -->
### Aggregate and `:with` binding

- Every variable appearing in an aggregate `(count ?x)` must be bound in `:where`.
- Every variable in `:with` must be bound in `:where`.
- `:with` requires at least one aggregate in `:find`.

### Window function compatibility

| Function | Requires `:over` | Allowed without `:over` |
|----------|-----------------|------------------------|
| `avg`, `rank`, `row-number` | Yes | No |
| `count-distinct`, `sum-distinct` | No | Yes (not window-compatible) |
| `count`, `sum`, `min`, `max` | Optional | Yes |
| UDF names | Optional | Yes (runtime-resolved) |

### UUID and timestamp validation

- `#uuid "..."` — the string must be a valid RFC 4122 UUID (e.g. `"550e8400-e29b-41d4-a716-446655440000"`).
- `:as-of "..."`, `:valid-at "..."`, `:valid-from "..."`, `:valid-to "..."` — the string must be a
  parseable ISO 8601 UTC timestamp (e.g. `"2024-01-15T10:00:00Z"`).
- `:as-of N` — the integer counter must be non-negative.

### Bind slots in attribute position

`$slot` is not permitted in the **attribute** position of a pattern when the query is
used with `db.prepare()`. The query optimizer selects an index based on the attribute at
prepare time and cannot handle a parameterised attribute.

---

Minigraf uses a Datalog dialect with EDN (Extensible Data Notation) syntax, inspired by Datomic and XTDB.

## Commands

All interaction goes through `db.execute(string)` or the interactive REPL.

```
(transact [ facts... ])                    — assert facts
(transact { options } [ facts... ])        — assert with valid-time options
(retract [ facts... ])                     — retract facts
(query [ :find vars :where clauses ])      — query
(rule [ (head args) body-clauses... ])     — define recursive rule
```

---

## Facts

A fact is a triple `[entity attribute value]` — optionally extended with valid time.

```datalog
;; Basic triple
[:alice :person/name "Alice"]
[:alice :person/age 30]
[:alice :friend :bob]           ;; value is an entity ref

;; Optional 4th element: per-fact valid time override
[:alice :employment/status :active {:valid-from "2023-01-01" :valid-to "2024-01-01"}]
;; [entity attribute value {:valid-from ... :valid-to ...}]
```

### Value types

| Type | Example |
|---|---|
| String | `"Alice"` |
| Integer | `42`, `-7` |
| Float | `3.14` |
| Boolean | `true`, `false` |
| Entity ref | `:bob` (keyword that resolves to an entity) |
| Keyword | `:status/active` |
| Null | `nil` |

Entities are UUIDs internally; you can use keywords as shorthand in the REPL (they are resolved to stable UUIDs per session).

---

## Transact

```datalog
;; Assert multiple facts atomically
(transact [[:alice :person/name "Alice"]
           [:alice :person/age 30]
           [:alice :friend :bob]])

;; With transaction-level valid time (applies to all facts in the batch)
(transact {:valid-from "2023-01-01" :valid-to "2024-06-30"}
          [[:alice :employment/status :active]])

;; Per-fact valid time override (4th element map; overrides a transaction-level map)
(transact [[:alice :employment/status :active {:valid-from "2023-01-01" :valid-to "2024-01-01"}]
           [:alice :employment/status :contractor {:valid-from "2024-01-01" :valid-to "2025-01-01"}]])
```

Valid-time values are ISO 8601 strings (`"2024-01-15"` or `"2024-01-15T10:00:00Z"`). Omitting `:valid-to` leaves it open-ended (valid forever).

<!-- @since v3.0.0 -->
### One current valid-time window per fact

At any transaction time, each `(entity, attribute, value)` has exactly one current valid-time window: the one from its latest `transact`. A later assertion of the same fact replaces the earlier window, which stays visible through `:as-of` of the earlier transaction. Closing, extending or reopening a window is a plain `transact` with the new bounds; `retract` withdraws the fact.

```datalog
(transact {:valid-from "2023-06-01"} [[:alice :works-at :startupco]])                       ;; open-ended
(transact {:valid-from "2023-06-01" :valid-to "2025-12-31"} [[:alice :works-at :startupco]])  ;; now closed

(query [:find ?co :valid-at "2026-03-01" :where [:alice :works-at ?co]])
;; => no rows: the closed window replaced the open one
```

A fact that was true over two separate periods cannot have both windows current at once. Model each period as its own entity, or read earlier periods with `:as-of`. Asserting one fact with two different windows in a single `transact` fails with `API-011` and writes nothing.

### Empty windows are rejected

A `transact` whose effective window for any fact ends at or before it starts fails with `API-019`, writes nothing and takes no transaction number. The effective window is the one after defaults, so `(transact {:valid-to "2020-01-01"} ...)` is rejected too: its `:valid-from` is the transaction time.
<!-- @end -->
<!-- @until v3.0.0 -->
### Known issues with valid time in v2.x

- A later assertion of the same `(entity, attribute, value)` with a different window does not replace the earlier window: every window ever asserted stays current until a `retract` ([#435](https://github.com/project-minigraf/minigraf/issues/435)). To close or change a window, retract the fact first, then assert it with the new window (see [Closing an open-ended fact](cookbook-bitemporal-modeling#recipe-7--closing-an-open-ended-fact)).
- A window that ends at or before it starts is stored, never matches `:valid-at`, and still shows up under `:any-valid-time`.
- Two values of one attribute written in the same transaction can read back as one value ([#371](https://github.com/project-minigraf/minigraf/issues/371)). Write them in separate transactions.

All are fixed in v3.0.0. Every v2.x known issue is listed in [#421](https://github.com/project-minigraf/minigraf/issues/421).
<!-- @end -->

---

## Retract

```datalog
;; Retract a specific fact triple
(retract [[:alice :friend :bob]])

;; Retract multiple facts
(retract [[:alice :person/age 30]
          [:alice :employment/status :active]])
```

Retraction records a new fact with `asserted = false`. The original fact remains in history and is visible via time-travel queries.

---

## Query

```datalog
;; Basic find
(query [:find ?name
        :where [?e :person/name ?name]])

;; Multi-clause join
(query [:find ?friend-name
        :where [:alice :friend ?friend]
               [?friend :person/name ?friend-name]])

;; Bind a specific entity
(query [:find ?age
        :where [:alice :person/age ?age]])
```

### Variables

Variables start with `?`. They are unified across clauses — the same variable in two clauses constrains them to the same value.

### Where clauses

Each clause is `[entity attribute value]`. Any position can be a variable, a literal, or a keyword entity ref.

```datalog
[?e :person/name ?name]      ;; bind entity and name
[:alice :friend ?friend]     ;; bind friend of alice
[?e :person/age 30]          ;; find all entities aged 30
```

---

## Bi-temporal Queries

Every fact has two time dimensions:

- **Transaction time** (`tx_count`) — when the fact was recorded in the database. Auto-managed, immutable, monotonic.
- **Valid time** (`valid_from` / `valid_to`) — when the fact was true in the real world. Set by the caller; can be backdated or pre-dated.

### `:as-of` — time travel by transaction time

```datalog
;; Query as of transaction counter 50
(query [:find ?status
        :as-of 50
        :where [:alice :employment/status ?status]])

;; Query as of an ISO 8601 timestamp (matches the tx_count recorded at that wall-clock time)
(query [:find ?status
        :as-of "2024-01-15T10:00:00Z"
        :where [:alice :employment/status ?status]])
```

### `:valid-at` — time travel by valid time

```datalog
;; Query facts that were valid on a specific date
(query [:find ?name
        :valid-at "2023-06-01"
        :where [:alice :person/name ?name]])
```

### `:max-derived-facts` and `:max-results` — per-query complexity limits

Override the database-level `OpenOptions` complexity limits for a single query:

```datalog
;; Run a transitive-closure query that would normally hit the 1M default
(query [:find ?ancestor
        :where (ancestor ?ancestor "abc123")
        :max-derived-facts 5000000
        :max-results 10000])
```

Both keys are optional and order-independent. Omitting a key falls back to the `OpenOptions`
value for that limit. Values must be positive integers (≥ 1).

- `:max-derived-facts N` — caps how many facts the recursive rule engine can derive
  internally before returning an error. Use when a legitimate recursive query exceeds
  the database default.
- `:max-results N` — caps the maximum number of result rows returned. Applies inside
  the rule evaluator; for non-recursive queries results are not truncated by this value.

The limits are applied for that query only and do not affect subsequent queries.

### Combined bi-temporal query

```datalog
;; Both axes at once: what did we record (tx time) about what was true (valid time)
(query [:find ?status
        :as-of "2024-01-15T10:00:00Z"
        :valid-at "2023-06-01"
        :where [:alice :employment/status ?status]])
```

### `:any-valid-time` — ignore valid-time filter

```datalog
;; Return facts regardless of valid time (see all versions)
(query [:find ?name :valid-at :any-valid-time :where [?e :person/name ?name]])
```

---

## Negation

### `not` — stratified negation

Exclude outer bindings where all body variables are pre-bound and the pattern matches.

```datalog
;; Exclude entities that have a :banned attribute
(query [:find ?e
        :where [?e :person/name ?name]
               (not [?e :banned true])])

;; Multi-clause not: exclude if BOTH patterns match
(query [:find ?e
        :where [?e :person/name ?name]
               (not [?e :role :admin]
                    [?e :active true])])

;; not in a rule body
(rule [(eligible ?x)
       [?x :applied true]
       (not (rejected ?x))])
```

All variables in a `not` body must be bound by outer clauses (safety / range-restriction). Nested `not` is rejected at parse time.

### `not-join` — existentially-quantified negation

Exclude outer bindings when there *exists* some assignment to inner-only variables that satisfies the body. Only the explicitly listed `join_vars` are shared from the outer binding; all other body variables are fresh/unbound.

```datalog
;; Exclude ?e if there exists any ?tag such that (?e :has-tag ?tag) and (?tag :is-bad true)
(query [:find ?e
        :where [?e :person/name ?name]
               (not-join [?e]
                         [?e :has-tag ?tag]
                         [?tag :is-bad true])])

;; Multiple join variables
(query [:find ?e
        :where [?e :name ?n]
               [?e :role ?r]
               (not-join [?e ?r]
                         [?e :has-role ?r]
                         [?r :is-admin true])])

;; not-join in a rule body
(rule [(eligible ?x)
       [?x :applied true]
       (not-join [?x]
                 [?x :dep ?d]
                 [?d :status :rejected])])
```

**Contrast with `not`**: `not` requires all body variables to be pre-bound by outer clauses. `not-join` allows inner variables (`?tag`, `?d` above) that are fresh — not mentioned in `join_vars` — to be existentially quantified.

**Safety constraint**: every variable in `join_vars` must be bound by an outer clause — enforced at parse time. Nesting `not-join` inside `not` or another `not-join` is rejected.

### Stratification

Negation uses stratified evaluation (Datalog^¬). The rule dependency graph is analysed at registration time:

- Rules with `not` / `not-join` bodies referencing a predicate `p` create a *negative dependency* on `p`.
- If a negative cycle is detected (e.g. rule A negates rule B and rule B negates rule A), `register_rule` returns an error and the rule is not added.
- Non-recursive negation is always safe.

```datalog
;; This is REJECTED — negative cycle: p not→ q, q not→ p
(rule [(p ?x) (not (q ?x))])
(rule [(q ?x) (not (p ?x))])
;; Error: [INT-054] unstratifiable: predicate 'q' is involved in a negative cycle through 'p'
```

---

## Disjunction

### `or` — match any branch

```datalog
;; Succeeds if any branch matches.
(or clause1 clause2 ...)

;; Use (and ...) to group multiple clauses into one branch:
(or (and clause1 clause2 ...) clause3 ...)
```

All branches must introduce the same set of new variables.

### `or-join` — existentially-quantified disjunction

```datalog
;; join_vars are shared with the outer query.
;; Variables inside branches but not in join_vars are private (existential).
(or-join [?v1 ?v2] branch1 branch2 ...)
(or-join [?v1] (and [?v1 :a ?priv1]) (and [?v1 :b ?priv2]))
```

All `join_vars` must be bound by preceding clauses. Branch-private variables do not appear in query results.

### Branch contents

Each branch (or a single clause) may contain any `WhereClause`: `Pattern`, `RuleInvocation`, `not`, `not-join`, `Expr`, and nested `or`/`or-join`.

### Safety

- `or`: all branches must introduce the same set of new variable names. Mismatched sets → parse error.
- `or-join`: all `join_vars` must be bound by a preceding clause → parse error if not.

---

## Aggregation

Scalar aggregates appear in the `:find` clause as `(func ?var)`. All aggregates skip `null` values silently (SQL semantics).

**Supported functions**: `count`, `count-distinct`, `sum`, `sum-distinct`, `min`, `max`

```datalog
;; count — total number of non-null bindings
(query [:find (count ?e)
        :where [?e :person/name _]])

;; count-distinct — distinct non-null values
(query [:find (count-distinct ?tag)
        :where [?e :note/tag ?tag]])

;; sum / sum-distinct
(query [:find ?dept (sum ?salary)
        :where [?e :dept ?dept]
               [?e :salary ?salary]])

;; min / max — works on Integer, Float, and String
(query [:find (min ?ts)
        :where [?e :event/timestamp ?ts]])
```

### Grouping

When the `:find` clause mixes plain variables and aggregates, the plain variables are grouping keys — one output row per unique combination.

```datalog
;; One row per department: ("eng" 3), ("hr" 1), …
(query [:find ?dept (count ?e)
        :where [?e :dept ?dept]])
```

### `:with` clause

`:with` adds extra variables to the grouping key without including them in the output. This prevents two rows with the same `:find` values but different identities from collapsing into one group before aggregation.

```datalog
;; Without :with — two employees with same dept+salary form one group
;; With :with ?e  — each employee gets its own group (finer-grained)
(query [:find ?dept (sum ?salary)
        :with ?e
        :where [?e :dept ?dept]
               [?e :salary ?salary]])
```

### Empty-result semantics

| Aggregate | No bindings, no grouping vars | No bindings, with grouping vars |
|-----------|-------------------------------|---------------------------------|
| `count` / `count-distinct` | `[[0]]` | empty set |
| `sum` / `min` / `max` | empty set | empty set |

### Type rules

- `sum` / `sum-distinct`: Integer inputs → Integer result; any Float input → Float result (widening). Non-numeric, non-null values → runtime error.
- `min` / `max`: Comparable types (Integer, Float, String). Mixing Integer and Float → runtime error. Non-comparable types (Boolean, Ref, Keyword) → runtime error.

---

## Window Functions

Window functions appear in the `:find` clause as `(func ?v :over (...))`. They annotate each result row with a computed value — unlike aggregates, they do **not** collapse rows.

### Syntax

```datalog
(func ?v :over (:partition-by ?p :order-by ?o))
(func ?v :over (:partition-by ?p :order-by ?o :desc))
(func ?v :over (:order-by ?o))          ;; no partition — whole result is one partition
(rank :over (:order-by ?o))             ;; no input variable for rank / row-number
(row-number :over (:order-by ?o))
```

`:partition-by` is optional. `:order-by` is required. `:desc` reverses the sort order (default is ascending).

### Supported window functions

| Function | Input variable | Semantics |
|---|---|---|
| `sum ?v :over (…)` | required | Cumulative sum from partition start to current row |
| `count ?v :over (…)` | required | Cumulative count from partition start to current row |
| `min ?v :over (…)` | required | Running minimum over rows seen so far in partition |
| `max ?v :over (…)` | required | Running maximum over rows seen so far in partition |
| `avg ?v :over (…)` | required | Running average over rows seen so far in partition |
| `rank :over (…)` | none | Rank within partition (ties share rank, next rank skips) |
| `row-number :over (…)` | none | Sequential 1-based row number within partition |

`lag` and `lead` are not supported in this version.

Frame semantics: all window functions use **unbounded-preceding** (accumulate from the first row in the partition up to and including the current row). Sliding frames (`:rows N preceding`) are deferred to a future release.

### Examples

```datalog
;; Cumulative salary sum ordered by hire date, partitioned by department
(query [:find ?e ?dept (sum ?salary :over (:partition-by ?dept :order-by ?hire))
        :where [?e :employee/dept ?dept]
               [?e :employee/salary ?salary]
               [?e :employee/hire-date ?hire]])

;; Rank employees within each department by salary (highest first)
(query [:find ?e ?dept (rank :over (:partition-by ?dept :order-by ?salary :desc))
        :where [?e :employee/dept ?dept]
               [?e :employee/salary ?salary]])

;; Row number over all events ordered by tx-count (bi-temporal use case)
(query [:find ?e (row-number :over (:order-by ?tx))
        :any-valid-time
        :where [?e :event/type :login]
               [?e :db/tx-count ?tx]])

;; Mix aggregate and window in same query (aggregate collapses first, window annotates after)
(query [:find ?dept (count ?e) (rank :over (:order-by ?dept))
        :where [?e :employee/dept ?dept]])
```

### Mixed aggregate + window queries

When a `:find` clause contains both plain aggregates (e.g. `(count ?e)`) and window functions (e.g. `(rank :over (...))`), aggregation runs first — collapsing rows into one per group — and then window functions annotate the collapsed rows.

### Type rules

- `sum` / `avg`: Integer inputs → Integer/Float result; any Float input → Float result. Non-numeric, non-null → runtime error.
- `min` / `max`: Comparable types (Integer, Float, String). Mixing Integer and Float → runtime error.
- `rank` / `row-number`: always return `Integer`.
- `count`: always returns `Integer`, counts non-null rows only.

---

## User-Defined Functions

Minigraf supports two categories of UDFs registered via the Rust API: **custom aggregate functions** and **custom predicate functions**. Both integrate with the standard query syntax.

### Custom Aggregate Functions

Register with `Minigraf::register_aggregate(name, init, step, finalise)`:

```rust
// Geometric mean: init = (product=1.0, count=0), step multiplies, finalise takes nth root
db.register_aggregate(
    "geomean",
    || Box::new((1.0f64, 0u64)),                         // init
    |state, val| {                                        // step
        let s = state.downcast_mut::<(f64, u64)>().unwrap();
        if let Value::Float(f) = val { s.0 *= f; s.1 += 1; }
        else if let Value::Integer(i) = val { s.0 *= *i as f64; s.1 += 1; }
    },
    |state| {                                             // finalise
        let s = state.downcast_ref::<(f64, u64)>().unwrap();
        if s.1 == 0 { Value::Null } else { Value::Float(s.0.powf(1.0 / s.1 as f64)) }
    },
)?;
```

Use in a `:find` clause exactly like any built-in aggregate:

```datalog
;; Grouping aggregate
(query [:find (geomean ?score)
        :where [?e :item/score ?score]])

;; With grouping variable
(query [:find ?dept (geomean ?score)
        :where [?e :employee/dept ?dept]
               [?e :employee/score ?score]])
```

Custom aggregates also work in `:over` window clauses:

```datalog
(query [:find ?e (geomean ?score :over (:partition-by ?dept :order-by ?score))
        :where [?e :employee/dept ?dept]
               [?e :employee/score ?score]])
```

### Custom Predicate Functions

Register with `Minigraf::register_predicate(name, f)`. The name must end with `?` by convention (not enforced):

```rust
db.register_predicate(
    "email?",
    |v| matches!(v, Value::String(s) if s.contains('@')),
)?;
```

Use in a `:where` clause as a single-argument filter:

```datalog
(query [:find ?e
        :where [?e :person/email ?addr]
               [(email? ?addr)]])
```

### Runtime Resolution

Unknown function names in `:find` aggregates, `:over` window clauses, and `[(name? ?var)]` filter expressions are **not rejected at parse time** — the parser emits `WindowFunc::Udf(name)`, `UnaryOp::Udf(name)`, etc. and defers validation to execution. If the name is not registered when the query runs, the executor returns an `Err` with a clear message.
<!-- @since v3.0.0 -->

Aggregate and window function names are resolved when the query starts, before any row is read: an unknown aggregate fails with `QRY-010` and an unknown window function with `QRY-011`, even on an empty database or when no row matches.
<!-- @end -->
<!-- @until v3.0.0 -->

An unknown aggregate or window function is only reported once a row reaches it (as `INT-029` / `INT-030`); a query where no row matches returns an empty result instead.
<!-- @end -->

This means queries referencing UDFs can be parsed and stored before the UDF is registered, and will succeed once registration occurs.

---

## Arithmetic & Predicate Expressions

Expression clauses appear in `:where` as a vector whose first element is a list `(op ...)`.

### Filter predicates

A filter clause keeps the current binding if the expression evaluates to a truthy value. No new variable is introduced.

```datalog
;; Comparison filters
[(< ?age 30)]
[(>= ?salary ?min-salary)]
[(= ?status :active)]
[(!= ?role :admin)]

;; Type predicate filters
[(string? ?name)]
[(integer? ?count)]
[(float? ?ratio)]
[(boolean? ?flag)]
[(nil? ?maybe)]

;; String predicate filters
[(starts-with? ?tag "work")]
[(ends-with? ?file ".rs")]
[(contains? ?bio "engineer")]
[(matches? ?email "^[^@]+@[^@]+$")]   ;; regex validated at parse time
```

### Arithmetic bindings

An arithmetic binding evaluates the expression and binds the result to an output variable.

```datalog
;; Basic arithmetic
[(+ ?price ?tax) ?total]
[(* ?price ?qty) ?subtotal]
[(- ?gross ?cost) ?profit]
[(/ ?total ?count) ?average]

;; Nested expressions
[(+ (* ?a 2) ?b) ?result]

;; Type predicate as binding (binds true or false)
[(integer? ?v) ?is-int]
[(string? ?v) ?is-str]
```

Combine with aggregation:

```datalog
(query [:find (sum ?total)
        :where [?e :order/price ?price]
               [?e :order/qty ?qty]
               [(* ?price ?qty) ?total]])
```

### Operators

**Comparison** (return `Boolean`): `<` `>` `<=` `>=` `=` `!=`

**Arithmetic** (return `Integer` or `Float`): `+` `-` `*` `/`

**String predicates** (return `Boolean`): `starts-with?` `ends-with?` `contains?` `matches?`

**Type predicates** (return `Boolean`): `string?` `integer?` `float?` `boolean?` `nil?`

### Semantics

- `<` `>` `<=` `>=` require both operands to be numeric (`Integer` or `Float`); type mismatch → row silently dropped
- `=` / `!=` use structural equality; type mismatch returns `false` / `true` (not an error)
- `Integer + Float` → `Float` (widening promotion); integer `/` integer → integer (truncation)
- Division by zero → row silently dropped; NaN result → row silently dropped
- `is_truthy`: `Boolean(true)`, non-zero `Integer`, non-zero `Float` → true; everything else → false
- `matches?` pattern is validated at parse time; an invalid regex is a parse error
- Expressions inside `not` / `not-join` bodies are evaluated correctly

### Safety check

All `?var` references in an expression clause must be bound by earlier `:where` clauses. An unbound variable is a parse-time error.

---

## Recursive Rules

Rules define named relations that can be used in queries. They support recursion, enabling graph traversal.

```datalog
;; Define a rule
(rule [(reachable ?from ?to)
       [?from :connected ?to]])

;; Base + recursive case
(rule [(reachable ?from ?to)
       [?from :connected ?intermediate]
       (reachable ?intermediate ?to)])

;; Use the rule in a query
(query [:find ?dest
        :where (reachable :node-a ?dest)])
```

A rule invocation takes one or two arguments, for example `(tagged ?e)` or `(reachable ?a ?b)`. An invocation with more arguments fails at query time with `INT-028`.

Rules use semi-naive fixed-point evaluation. Cycles in the graph are handled correctly — the evaluator converges on the fixed point without infinite loops.

When a query binds at least one argument to a recursive rule, Minigraf automatically applies **magic sets rewriting** — the query is rewritten top-down to propagate bound values into the recursion, avoiding full-relation scans. No user action is required; the optimisation is transparent. Note: mutual recursion through negation is not rewritten and falls back to bottom-up evaluation.

### Family tree example

```datalog
(rule [(ancestor ?a ?d) [?a :parent ?d]])
(rule [(ancestor ?a ?d) [?a :parent ?m] (ancestor ?m ?d)])

(query [:find ?ancestor
        :where (ancestor :charlie ?ancestor)])
```

---

## Prepared Statements (Rust API)

Parse and plan a query once; execute it many times with different bind values — including temporal filters — without re-parsing or re-planning on each call.

### Syntax

`$identifier` tokens (dollar sign + identifier) are **bind slots** — named parameters substituted at execute time:

```datalog
;; Entity slot, as-of slot, valid-at slot
(query [:find ?status
        :as-of $tx
        :valid-at $date
        :where [$entity :employment/status ?status]])
```

### Bind slot positions and permitted types

| Position | Permitted `BindValue` variants |
|---|---|
| Entity in pattern (`[$entity :attr ?v]`) | `Entity(Uuid)` |
| Value in pattern (`[?e :attr $val]`) | `Val(Value)` |
| `:as-of $tx` | `TxCount(u64)`, `Timestamp(i64)` |
| `:valid-at $date` | `Timestamp(i64)`, `AnyValidTime` |

**Attribute position is not parameterisable** — substituting the attribute name at execute time would make it impossible to select the correct index at prepare time, defeating plan reuse.

### API

```rust
use minigraf::{Minigraf, OpenOptions, BindValue};

let db = OpenOptions::new().path("agents.graph").open()?;

// Prepare once — parses, validates, and computes the query plan
let pq = db.prepare(
    "(query [:find ?belief
             :as-of $tx
             :where [$entity :belief/value ?belief]])"
)?;

// Execute many times — plan is reused, only bind values are substituted
let r1 = pq.execute(&[
    ("tx",     BindValue::TxCount(50)),
    ("entity", BindValue::Entity(alice_id)),
])?;

let r2 = pq.execute(&[
    ("tx",     BindValue::TxCount(75)),
    ("entity", BindValue::Entity(bob_id)),
])?;
```

### Notes

- `PreparedQuery` holds `Arc` clones of the live fact store — each `execute()` sees the current state (including facts transacted after `prepare()`)
- `db.execute(str)` string API is unchanged — no breaking change
- `BindValue::AnyValidTime` is the programmatic equivalent of writing `:any-valid-time` in the query string
<!-- @since v3.0.0 -->

### Cursors

`db.query(str)` and `pq.query(&binds)` return a `Cursor` that hands rows back in batches: `vars()`, `next_batch(max_rows)`, `close()`, and an iterator over rows. The answer is fixed when the cursor opens, so writes that commit while it is open do not change it. A cursor borrows nothing from the database handle: it can outlive it and move to another thread. For now the answer is still computed when the cursor opens, under the same limits as `execute()`.

```rust
let mut cursor = db.query("(query [:find ?n :where [?e :person/name ?n]])")?;
while let Some(batch) = cursor.next_batch(1000)? {
    for row in batch.rows() {
        // ...
    }
}
```

`query()` accepts only queries: `transact`, `retract` and `rule` fail with `API-012`, and `db.query()` with bind slots fails with `API-010` (use `prepare()`). In the browser, `BrowserDb.query(datalog)` returns a `BrowserCursor` with `vars()`, `nextBatch(maxRows)` and `close()`.
<!-- @end -->

---

## Explicit Transactions (Rust API)

```rust
let mut tx = db.begin_write()?;
tx.execute(r#"(transact [[:alice :person/age 31]])"#)?;
tx.execute(r#"(retract [[:alice :person/age 30]])"#)?;
tx.commit()?;   // or tx.rollback()
```

All operations within a `WriteTransaction` are atomic. On rollback or drop without commit, all changes are discarded.

<!-- @since v3.0.0 -->
When several statements in one `WriteTransaction` write the same `(entity, attribute, value)`, in any mix of `transact` and `retract`, the last of them decides it at commit. A fact retracted and then asserted again in one transaction is live after `commit()`, as it reads inside the transaction. Two windows of one fact in separate statements commit the later window. Two windows of one fact inside one `transact` fail with `API-011`; `tx.execute()` stages nothing for that statement and the transaction stays usable.
<!-- @end -->
<!-- @until v3.0.0 -->
Known issue ([#477](https://github.com/project-minigraf/minigraf/issues/477)): the commit stamps every record with one transaction number, and a retraction always wins. A fact retracted and then asserted again in one `WriteTransaction` reads as live before `commit()` and is gone after it. Fixed in v3.0.0.
<!-- @end -->

---

## REPL Commands

Run `cargo run --bin minigraf` to start the interactive REPL.

```
minigraf> (transact [[:alice :person/name "Alice"]])
minigraf> (query [:find ?name :where [?e :person/name ?name]])
minigraf> (rule [(friend-of-friend ?a ?c) [?a :friend ?b] [?b :friend ?c]])
```

Multi-line input is supported — press Enter on an incomplete expression to continue on the next line. Lines starting with `;` are comments. `Ctrl-D` exits.

---

## Constraints and Limits

<!-- @until v3.0.0 -->
- **Max fact size (file-backed)**: 4 080 serialised bytes per fact. Facts that exceed this are rejected at insertion with a clear error. In-memory databases have no limit.
<!-- @end -->
<!-- @since v3.0.0 -->
- **Max value sizes (file-backed)**: a string value up to 4,068 bytes, whatever the rest of the fact; an attribute name or keyword value up to 1,024 bytes. Both are checked when the transaction is written and fail with `WAL-003`, which names the value. In-memory databases have no limit.
<!-- @end -->
- **Entities**: UUIDs internally; keywords in the REPL resolve to stable per-session UUIDs.
- **Timestamps**: UTC only (`"2024-01-15T10:00:00Z"` or date-only `"2024-01-15"`).
- **Rule arity**: rule invocations take one or two arguments (`INT-028` otherwise).
- **Stratified negation** (`not` / `not-join`) supported.
- **Scalar aggregation** (`count`, `count-distinct`, `sum`, `sum-distinct`, `min`, `max`, `:with`) supported.
- **Arithmetic & predicate expressions** (`[(< ?v 100)]`, `[(+ ?a ?b) ?c]`, string/type predicates) supported.
- **Disjunction** (`or` / `or-join`) supported — see [Disjunction](#disjunction) above.
- **User-defined aggregate functions** (`register_aggregate`) usable in `:find` grouping and `:over` window clauses — see [User-Defined Functions](#user-defined-functions) above.
- **User-defined predicate functions** (`register_predicate`) usable in `[(name? ?var)]` `:where` clauses — see [User-Defined Functions](#user-defined-functions) above.
- **Prepared statements** (`db.prepare(str)` → `PreparedQuery`) — parse + plan once, execute many times with named `$slot` bind values — see [Prepared Statements](#prepared-statements-rust-api) above.
