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

<!-- @since v3.0.0 -->
The structural syntax accepted by the parser is on the [Grammar](grammar) page: EBNF
generated from the grammar the parser is tested against, so it matches each release.
Semantic constraints (safety checks, binding rules, compatibility rules) are documented
separately in the [Semantic Constraints](#semantic-constraints) section below.

<!-- @end -->
<!-- @until v3.0.0 -->
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
aggregate-expr ::= "(" (op-sym | symbol) variable ")"
window-expr    ::= "(" (op-sym | symbol) (variable ":over" | ":over") over-clause ")"
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
expr        ::= "(" (unary-op | op-sym | symbol) expr-arg ")"
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
            | keyword | op-sym | variable | symbol | list | vector | map
list      ::= "(" edn-value* ")"
vector    ::= "[" edn-value* "]"
map       ::= "{" (edn-value edn-value)* "}"

(* ── Primitives ───────────────────────────────────────────────────────── *)
keyword   ::= ":" word-char+
op-sym    ::= "<=" | ">=" | "!=" | "<" | ">" | "=" | "+" | "*" | "/"
symbol    ::= (letter | "_") word-char*    (* except boolean and nil *)
            | "-" word-char+                 (* not followed by a numeral after "-" *)
variable  ::= "?" word-char*
boolean   ::= ("true" | "false")           (* not followed by a word-char *)
nil       ::= "nil"                        (* not followed by a word-char *)
integer   ::= "-"? digit+
float     ::= "-"? digit+ "." digit*
string    ::= '"' str-char* '"'
str-char  ::= "\" any-char                 (* \n \t \r \" \\ are escapes; before any other char, "\" is dropped *)
            | any-char-except-quote-backslash
uuid      ::= "#uuid" string
bind-slot ::= "$" (letter | numeral | "_" | "-")+

(* letter = a Unicode Alphabetic character, numeral = a character in Unicode's number
   categories (Nd, Nl, No), digits included; digit = [0-9] *)
(* word-char = letter | numeral | "?" | "_" | "-" | "/" *)
(* Whitespace (space, tab, newline, carriage return, comma) and comments (";" to the
   end of the line) are ignored between tokens. *)
```

<!-- @end -->

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

<!-- @until v3.0.0 -->
| Function | Requires `:over` | Allowed without `:over` |
|----------|-----------------|------------------------|
| `avg`, `rank`, `row-number` | Yes | No |
| `count-distinct`, `sum-distinct` | No | Yes (not window-compatible) |
| `count`, `sum`, `min`, `max` | Optional | Yes |
| UDF names | Optional | Yes (runtime-resolved) |
<!-- @end -->
<!-- @since v3.0.0 -->
| Function | Requires `:over` | Allowed without `:over` |
|----------|-----------------|------------------------|
| `avg`, `rank`, `row-number` | Yes | No |
| `count-distinct`, `sum-distinct` | No | Yes (not window-compatible) |
| `count`, `sum`, `min`, `max` | Optional | Yes |
| UDF names | Optional | Yes (runtime-resolved) |
| `lag`, `lead` | Yes as built-ins | Yes when registered as UDF aggregates |
<!-- @end -->

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

<!-- @since v3.0.0 -->
A value can also be a **set** of the scalar values above, written `#{...}`:
`[:alice :user/scopes #{:scope/read :scope/write}]`. See [Sets](#sets).

<!-- @end -->

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

<!-- @until v3.0.0 -->
> **v2.x:** two values of one entity and attribute written in one `transact`, as in the last example, read back as one value ([#371](https://github.com/project-minigraf/minigraf/issues/371), fixed in v3.0.0). Write each in its own `transact`.
<!-- @end -->

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

<!-- @since v3.0.0 -->
### The query vector

A query vector is a run of sections, each a keyword and what follows it: `:find` and its
specs, `:where` and its clauses, `:with` and its variables, and the options `:as-of`,
`:valid-at`, `:any-valid-time`, `:max-derived-facts`, `:max-results`, `:order-by`,
`:limit`, `:offset` and `:distinct`. `:any-valid-time` and `:distinct` are bare flags;
the other options take arguments. Sections come in any order. A section with
nothing in it fails with `PRS-112`, a keyword the query does not know (such as `:foo`)
with `PRS-113`, and a value after an
option's argument with `PRS-061`: write `[:find ?a ?b :any-valid-time :where …]`, not
`[:find ?a :any-valid-time ?b :where …]`.

<!-- @end -->
### Variables

Variables start with `?`. They are unified across clauses — the same variable in two clauses constrains them to the same value.

### Repeated result rows

A query without aggregates returns one row per matching `:where` binding. Projecting
away a variable can therefore repeat a `:find` tuple: if `:e1` has two `:c` values
and `:e2` has one, `[:find ?e :where [?e :c ?v]]` returns three rows, including
`:e1` twice. This bag default also applies when the bindings come from rules.
`count-distinct` and `sum-distinct` apply to their aggregate argument only.

<!-- @until v3.0.0 -->
There is no option to remove repeated output rows. Deduplicate in the caller, or
add an aggregate to group by the projected variables:

```datalog
(query [:find ?e (count ?v) :where [?e :c ?v]])
```

This returns one row per entity but adds a count column. `:with` changes aggregate
grouping; it does not enable distinct output.
<!-- @end -->

<!-- @since v3.0.0 -->
### Distinct result rows

Add the bare `:distinct` flag to return one row per unique final `:find` tuple:

```datalog
(query [:find ?e :where [?e :c ?v] :distinct])
(query [:find ?v :where [?e :c ?v] :distinct :order-by [?v] :offset 1 :limit 10])
```

The flag takes no argument and can appear between complete query sections in any
order. Repeating it is `INT-012`; writing `:distinct true`, `:distinct 1` or
`:distinct $d` is `PRS-061`. Its grammar section is `distinct-section ::= ":distinct"`.

Distinct runs **after aggregates and windows, before ordering and pagination**.
It compares every output column, ignoring variables absent from `:find`. Aggregate
inputs, window peers and ranks, and `:with` grouping stay the same. Equal output
tuples from different `:with` groups or window partitions collapse. In particular,
`row-number` restarts in each partition and can repeat if no other output column
distinguishes the rows.

Equality follows aggregate grouping: an integer differs from a float, floating
zeros are equal, all NaNs are one value, and a keyword differs from a ref to its
named entity. Sets compare by their elements. Returned float zeros use `-0.0` and
NaNs use one canonical NaN, including inside sets, even for singleton results.
This makes their representation independent of which binding arrives first.
Without `:order-by`, the order of unique rows is unspecified.

The engine retains a key for every unique tuple encountered, charged to
`OpenOptions::query_memory_limit` (`QRY-012` when exceeded). This state spans cursor
batches. With `:order-by` and `:limit`, the sort retains only its best rows, but distinct
still retains all unique keys it encounters. Prepared queries, cursors and reads
inside write transactions support the same flag. Omitting it keeps bag output.
<!-- @end -->

### Where clauses

Each clause is `[entity attribute value]`. Any position can be a variable, a literal, or a keyword entity ref.

```datalog
[?e :person/name ?name]      ;; bind entity and name
[:alice :friend ?friend]     ;; bind friend of alice
[?e :person/age 30]          ;; find all entities aged 30
```

<!-- @range v2.0.11..v3.0.0 -->
A float literal passed to a rule compares by IEEE `==`: `(xval ?e 0.0)` returns
both `0.0` and `-0.0` rows in every run
([#750](https://github.com/project-minigraf/minigraf/issues/750)).
<!-- @end -->

<!-- @since v3.0.0 -->
### Join rules

The answer of a join does not depend on the order the planner runs the patterns in:

- **Floats** are equal by IEEE `==` when a variable joins two patterns: `0.0` equals `-0.0`, and NaN equals nothing. A float *constant* in a pattern, written or bound to a `$slot`, matches equal floats only, with the same equality (#706).
- **Keyword or ref.** A keyword value equals a ref to the entity it names. When one variable is bound by several patterns, its value comes from the first of them in the `:where` text: `[?a :friend ?b] [?b :name "Bob"]` returns `?b` as the keyword `:friend` stored, and `[?b :name "Bob"] [?a :friend ?b]` as the entity's ref. Pseudo-attribute patterns (`:db/valid-at` included) never give the value, even when they run first.
- **Expression bindings unify.** An expression binding (`[(f …) ?v]`) whose variable another clause also binds keeps only the rows where its result equals that value, with the same equality, wherever it runs (see [Arithmetic bindings](#arithmetic-bindings)). Expressions count among the binders: the value comes from the first clause in the `:where` text that binds the variable.

The keyword-or-ref rule applies to the top-level `:where` of a query; `or` branches, `not` bodies and rule bodies keep the 2.x choice of kind. Expression unification applies everywhere.
<!-- @end -->

### Pseudo-attributes

<!-- @until v3.0.0 -->
`:db/valid-from`, `:db/valid-to`, `:db/tx-count` and `:db/tx-id` (which need `:valid-at :any-valid-time`) read the metadata of a fact matched by another pattern on the same entity: `[?e :person/age ?a] [?e :db/tx-count ?t]` binds the tx-count of the `:person/age` fact. With several patterns on that entity, the fact comes from the last of them the planner runs before the pseudo-attribute pattern, so the answer can depend on the plan ([#594](https://github.com/project-minigraf/minigraf/issues/594)). A pseudo-attribute pattern with no such pattern before it matches every visible fact of its entity, one row each. `:db/valid-at` binds the query's valid time (`nil` under `:any-valid-time`), once per visible fact of the entity.
<!-- @end -->

<!-- @since v3.0.0 -->
`:db/valid-from`, `:db/valid-to`, `:db/tx-count` and `:db/tx-id` (which need `:valid-at :any-valid-time`) read the metadata of one fact: the fact matched by their **source**, the nearest pattern **before** them in the `:where` text with the same entity term and a stored attribute (a keyword, or a variable as in `[?e ?a _]`). `[?e :person/name ?n] [?e :person/age ?a] [?e :db/tx-count ?t]` binds the tx-count of the `:person/age` fact, whatever order the planner runs the patterns in.

- **Same entity term.** The same variable, the same `$slot`, or constants naming the same entity (`:alice` and its UUID). `?e` and `:alice` are not the same term even when `?e` is `:alice`; `_` and `?_x` are never the same as anything.
- **Scope.** The search starts in the pseudo-attribute's own clause list, then continues before each enclosing `not` or `or`. It crosses a `not-join` or `or-join` only for an entity variable listed in its join variables. It never searches a sibling `or` branch, and stops at a rule body: a rule never reads its caller's patterns.
- **Inside `not` / `or`.** A pseudo-attribute reads the outer fact: `[?e :person/name ?n] (not [?e :db/tx-count 1])` excludes the entities whose `:person/name` fact is from transaction 1.
- **No source** is a parse-time error, `PRS-081`. To read every fact of an entity, put `[?e ?a _]` before the pseudo-attribute pattern: one row per fact.
- The value position binds the metadata, or checks it when it is a constant or a variable bound elsewhere.
- **Derived facts.** A fact a rule derives has no metadata: when the source matched one, the pseudo-attribute binds `nil` (the row is kept) and a constant never matches it. `(reach ?x ?y) [?x :reach ?y] [?x :db/tx-count ?t]` gives `nil` for derived `:reach` facts and the tx-count of a stored one. Inside a rule body the source is a stored fact or a fact derived by another rule, read the same way.

`:db/valid-at` binds the query's valid time (`nil` under `:any-valid-time`), once per visible fact of the entity; it needs no source.
<!-- @end -->

---

<!-- @since v3.0.0 -->
## Ordering and limiting results

```datalog
;; The ten people with the most friends, most first
(query [:find ?name (count ?f)
        :where [?p :person/name ?name] [?p :friend ?f]
        :order-by [(count ?f) :desc ?name]
        :limit 10])

;; The third page of ten, by name
(query [:find ?name ?age
        :where [?p :person/name ?name] [?p :person/age ?age]
        :order-by [?name]
        :offset 20
        :limit 10])
```

- `:order-by [k …]` orders the rows the query returns. Each entry is a `:find` element,
  a plain variable or an aggregate or window written as it appears in `:find`, optionally
  followed by `:asc` (the default) or `:desc`. Ties are broken by the other `:find`
  columns, in `:find` order, ascending, so every position of an ordered answer is decided
  and every plan returns the same list.
- Values compare as window functions and `min`/`max` compare them: numbers by value (an
  integer before an equal float, `-0.0` before `0.0`, NaN after every number), then
  strings, booleans, refs, keywords, `nil` and sets.
- `:limit N` returns at most `N` rows; `:limit 0` returns none. Without `:order-by`,
  which rows is unspecified and may differ between runs and plans (they are always rows
  of the whole answer), and the query stops reading once it has them: over ten million
  facts, `[:find ?e ?v :where [?e :a ?v] :limit 10]` reads one internal batch.
- `:offset N` skips the first `N` rows of the ordered answer, before `:limit`. It needs
  `:order-by` (`PRS-116`): without an order, pages could overlap or miss rows. To read a
  whole answer in pieces, use a cursor (`Minigraf::query`).
- With `:order-by`, the query reads its whole answer before it returns a row. Under a
  `:limit`, it keeps only the best `offset + limit` rows as it reads, so its memory
  follows the limit, not the answer.
- They apply after aggregates and windows, and to rule queries, cursors and prepared
  queries: `:limit $n` and `:offset $o` take `BindValue::Val(Value::Integer(n))`, `n >= 0`.
  They do not change `:max-derived-facts` or what rules derive.
- Errors: a value that is not an integer `>= 0` or a slot is `PRS-115`; an `:order-by`
  entry that is not a `:find` element is `PRS-117` (a variable bound in `:where` but not
  in `:find` included); an `:order-by` that is not a non-empty vector, names an element
  twice, or has a misplaced or unknown keyword is `PRS-118`; a section given twice is
  `INT-012`, one with no value `INT-013`.

Inside a window's `:over`, `:order-by` is the window's own and is unchanged.

---

<!-- @end -->
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
        :max-derived-facts 5000000])
```

Both keys are optional and order-independent. Values must be positive integers (≥ 1).
<!-- @until v3.0.0 -->
Omitting a key falls back to the `OpenOptions` value for that limit.

- `:max-derived-facts N` — caps how many facts the recursive rule engine can derive
  internally before returning an error. Use when a legitimate recursive query exceeds
  the database default.
<!-- @end -->
<!-- @until v2.0.5 -->
- `:max-results N` — caps the facts the rule evaluator holds, **counting the database's
  own live facts**, so on a database with `N` or more live facts every rule query fails
  with `INT-020` (#528). Raise it above the live facts plus the derived ones. Queries
  without rules are not limited by this value.
<!-- @end -->
<!-- @range v2.0.5..v3.0.0 -->
- `:max-results N` — caps how many facts the rules of a query may derive in total;
  the database's own facts do not count. Queries without rules are not limited by
  this value, and results are never truncated: the query fails with `INT-020`.
<!-- @end -->
<!-- @since v3.0.0 -->
There is one limit (#553):

- `:max-derived-facts N` — caps how many facts the rules of a query may derive in
  total, across iterations; the database's own facts do not count. A query that
  derives more fails with `INT-020`. Queries without rules are not limited by it,
  and results are never truncated.
- `:max-results N` — deprecated alias of `:max-derived-facts`. When both are given,
  the lower applies.

Either key replaces the `OpenOptions` limit for that query; with neither, the
`OpenOptions` limit applies. Before v3.0.0, `:max-derived-facts` capped the facts
derived in one iteration and `:max-results` the total.
<!-- @end -->

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

<!-- @since v3.0.0 -->
### Comparing with the outer row

A body takes from the outer row every variable it names (`not`) or its join variables (`not-join`), and compares them with the facts it matches as joins compare them (see [Join rules](#join-rules)): floats by IEEE `==`, so `0.0` equals `-0.0` and NaN equals nothing, and a keyword equals a ref to the entity it names. This holds whatever the body holds (patterns, expressions, rule calls) and wherever it appears (a query, an `or` branch, a rule body). A float *constant* written in the body matches equal floats only ([#706](https://github.com/project-minigraf/minigraf/issues/706)). A `:db/valid-at` pattern in a body binds the query's valid time, as it does outside.

An expression that fails on a body row (a type mismatch, a division by zero) drops that row, so it cannot exclude the outer row. An unknown predicate in a body fails the query with `QRY-007` when it opens. Before v3.0.0 it failed on every body row, so the `not` kept every outer row ([#551](https://github.com/project-minigraf/minigraf/issues/551)).
<!-- @end -->
<!-- @until v3.0.0 -->
A body whose only clauses are patterns compares the outer row's values as joins do. A body with an expression, a body in an `or` branch or rule, or one in a query that calls a rule, substitutes each outer value into its patterns as a constant, so an imported float matches within `f64::EPSILON` ([#546](https://github.com/project-minigraf/minigraf/issues/546)).
<!-- @end -->

### Stratification

Negation uses stratified evaluation (Datalog^¬). The rule dependency graph is analysed at registration time:

- Rules with `not` / `not-join` bodies referencing a predicate `p` create a *negative dependency* on `p`.
- If a negative cycle is detected (e.g. rule A negates rule B and rule B negates rule A), `register_rule` returns an error and the rule is not added.
- Non-recursive negation is always safe.
<!-- @since v2.0.8 -->
- A rule that calls a rule with `not`, `not-join`, `or` or `or-join` is evaluated in a later stratum than the rule it calls, so it sees all of that rule's rows. Before v2.0.8 the caller could run first and return too few rows (#628). Recursion through such a rule, where the called rule calls back, is repeated until it derives nothing new; before v2.0.8 it stopped after one step and returned too few rows (#633).
<!-- @end -->

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

<!-- @since v3.0.0 -->
### How branches meet the outer row

An `or` and an `or-join` run after every other clause of their clause list except the `not`s that need what they bind, in text order. For each outer row, a branch takes from it every variable the branch names (`or`) or its join variables (`or-join`), and compares them with the facts it matches as joins compare them (see [Join rules](#join-rules)): floats by IEEE `==`, and a keyword equals a ref to the entity it names. The outer row's value is the one kept. A branch that does not name a variable constrains nothing about it: `(or-join [?e] [?x :a 1] [?e :b 2])` keeps every outer row when some entity has `:a 1`. The rows an `or` / `or-join` returns are distinct: two rows with equal values in every variable bound so far are one row (`0.0` equals `-0.0`; a keyword never equals a ref).

Inside a branch, clauses run in the order they do in a `:where`, so a `not` or nested `or` in a branch sees a variable an expression of the branch binds. Over `[:x :n 1] [:y :n 2] [:z :n 5]`, `[:find ?e :where [?e :n ?v] (or-join [?e ?v] (and [(+ ?v 1) ?w] (not [?f :n ?w])) [?e :n 5])]` returns `:y` and `:z`. Before v3.0.0 a branch ran its `not`s before its expressions, so that `not` saw `?w` unbound, matched any fact, and the query returned `:z` only ([#551](https://github.com/project-minigraf/minigraf/issues/551)).
<!-- @end -->
<!-- @until v3.0.0 -->
A branch with no `not`, no expression reading an outer variable and no pseudo-attribute reading an outer pattern is evaluated alone and joined to the outer rows by exact value equality, so a keyword never meets a ref to the entity it names, branches naming different outer variables meet no outer row, and an `or-join` branch that does not bind every join variable matches nothing. Other branches start from each outer row and compare as joins do ([#546](https://github.com/project-minigraf/minigraf/issues/546)).
<!-- @end -->

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

<!-- @since v3.0.0 -->
### Exact, order-independent results

An aggregate's answer does not depend on the order in which the query produces its rows:

- `sum` / `sum-distinct` over integers add exactly and saturate to the `i64` range once, at the end: `i64::MAX + 1 + -1` is `i64::MAX`.
- With any float, the sum is computed exactly and rounded once to the nearest float, so `1e16 + 1.0 + -1e16 + 1.0 + 0.5` is `2.5` in every order. A NaN, or both infinities, gives NaN; otherwise an infinity gives that infinity. The sum is `-0.0` only when every value is `-0.0`.
- `min` / `max` of floats: `-0.0` is below `0.0`, and NaN is above every other number.
- `count-distinct` / `sum-distinct` treat `0.0` and `-0.0` as one value, and all NaNs as one.
- A grouping variable likewise puts `0.0` and `-0.0` in one group, and all NaNs in one; the group returns the least of its values in that order, so `-0.0` when the group holds it.
- When several groups fail, the query reports the first failing aggregate (in `:find` order) of the first failing group (in output order).

A [custom aggregate](#custom-aggregate-functions) receives its values in the order the query produces them, which can depend on the query plan.

<!-- @end -->
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

<!-- @until v3.0.0 -->
`lag` and `lead` are not supported in this version.
<!-- @end -->
<!-- @since v3.0.0 -->
`lag` and `lead` are also supported. `(lag ?v :over (:order-by ?o))` reads `?v` from the immediately preceding row in the ordered partition; `lead` reads the next row. Both require a value argument and return `nil` at a partition edge. They accept the same optional `:partition-by ?p` and `:desc` modifiers. There is no offset or explicit default argument in v3.0.0. Without `:over`, `(lag ?v)` and `(lead ?v)` remain names a user may register as ordinary aggregates. With `:over`, the built-in runs even if a user aggregate has the same name.
<!-- @end -->

<!-- @until v3.0.0 -->
Frame semantics: all window functions use **unbounded-preceding** (accumulate from the first row in the partition up to and including the current row). Sliding frames (`:rows N preceding`) are deferred to a future release.

Rows with equal `:order-by` values keep the order the query produced them in, so their `row-number` and running values can depend on the query plan. Keywords, refs and booleans do not sort: any two of them compare equal.

<!-- @end -->
<!-- @since v3.0.0 -->
#### Order, ties and peers

Each partition is sorted by its `:order-by` value under one order of all values: numbers by numeric value (an integer before an equal float, `-0.0` before `0.0`, NaN after every other number), then strings, booleans (`false` first), refs, keywords (by text) and `nil`. `:desc` reverses it.

Rows with equal `:order-by` values are **peers**:

- `rank` gives peers the same rank; the next rank skips.
- `sum`, `count`, `min`, `max`, `avg` and custom window functions accumulate from the first row of the partition through the **last peer** of the current row, so peers get the same value (SQL's default frame). With `?s` of 1.5, 5, 10, 10, `(sum ?s :over (:order-by ?s))` gives 1.5, 6.5, 26.5, 26.5.
- `row-number` gives each peer its own number, in the order of the other variables `:find` names; peers equal in all of them are identical rows, so which gets which number does not show in the answer.
- `lag` and `lead` use that same row order, including the tie break. They read adjacent rows, even when those rows are peers. With `:desc`, the row `lag` reads has a higher or equal order key. A neighboring `nil` value also yields `nil`.

The answer is the same on every query plan.

<!-- @end -->

<!-- @since v3.0.0 -->
#### Bounded row frames

Add `:rows N preceding` inside `:over` to include the current row and at most `N` preceding **physical rows** in the sorted partition. `N` is a nonnegative integer literal up to 9223372036854775807. `:rows 0 preceding` uses only the current row. Frames truncate at the start of a partition and reset for each partition. `:rows` and the other `:over` options can appear in any order.

```datalog
(avg ?v :over (:partition-by ?p :order-by ?o :rows 2 preceding))
```

Unlike the default peer frame, a bounded frame treats rows with equal `:order-by` values as separate positions. The existing tie break by named `:find` variables decides their order, including for `:desc`. `sum`, `count`, `min`, `max`, `avg`, and registered aggregate functions accept a bounded frame. `rank`, `row-number`, `lag`, and `lead` reject one with `PRS-125`; malformed or repeated frame options return `PRS-124`.

```datalog
;; Each employee's salary and average of their salary plus up to two earlier hires.
(query [:find ?e ?dept ?hire ?salary
              (avg ?salary :over (:partition-by ?dept :order-by ?hire :rows 2 preceding))
        :where [?e :employee/dept ?dept]
               [?e :employee/salary ?salary]
               [?e :employee/hire-date ?hire]])
```

Rows with `nil` or nonnumeric values still occupy positions. `count` skips `nil`; `sum` and `avg` skip nonnumbers; `min` and `max` skip `nil`. A registered aggregate receives every value, including `nil`. Built-in bounded windows run in linear time after sorting. Registered aggregates without an inverse replay each frame, taking up to `O(R × min(N + 1, R))` steps for `R` rows in a partition; a frame covering the whole prefix uses a linear running accumulator. Large middle-sized frames can invoke the user function many times. The engine memory limit accounts for retained window results and built-in state, but cannot account for memory allocated inside a user function.
<!-- @end -->

### Examples

<!-- @since v3.0.0 -->
```datalog
;; Previous and next salary in each department, ordered by hire date.
(query [:find ?e ?dept ?hire ?salary
              (lag ?salary :over (:partition-by ?dept :order-by ?hire))
              (lead ?salary :over (:partition-by ?dept :order-by ?hire))
        :where [?e :employee/dept ?dept]
               [?e :employee/salary ?salary]
               [?e :employee/hire-date ?hire]])
```
<!-- @end -->

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

<!-- @since v3.0.0 -->
A window in such a query reads the grouped rows, which hold only the plain `:find` variables, so its argument, `:partition-by` and `:order-by` must be plain `:find` variables; anything else fails to parse with `PRS-082`. `:with` variables do not group when `:find` holds a window.

<!-- @end -->
<!-- @until v3.0.0 -->
A window variable that is not a plain `:find` variable reads `nil` on every grouped row.

<!-- @end -->

### Type rules

- `sum`: Integer while every number so far is an integer, otherwise Float. `avg`: Float. Both skip values that are not numbers.
<!-- @until v3.0.0 -->
- `min` / `max`: compare Integer, Float and String values; of two values they cannot compare, the first seen is kept.
<!-- @end -->
<!-- @since v3.0.0 -->
- `min` / `max`: any values, by the order above.
<!-- @end -->
- `rank` / `row-number`: always return `Integer`.
- `count`: always returns `Integer`, counts non-null rows only.
<!-- @since v3.0.0 -->
- `lag` / `lead`: return the adjacent row's value, of any type, or `nil` at a partition edge.
<!-- @end -->

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

<!-- @since v3.0.0 -->
The names `lag` and `lead` are exceptions in a window clause: `:over` selects the built-in even if a custom aggregate with that name is registered. Their plain aggregate forms still call the registered function.
<!-- @end -->

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

Aggregate and window function names are resolved when the query starts, before any row is read: an unknown aggregate fails with `QRY-010` and an unknown window function with `QRY-011`, even on an empty database or when no row matches. So is every predicate the query names, at any depth (an `or` branch, a `not` / `not-join` body, an `or` branch of a rule the query evaluates): an unknown one fails with `QRY-007` ([#551](https://github.com/project-minigraf/minigraf/issues/551)).
<!-- @end -->
<!-- @until v3.0.0 -->

An unknown aggregate or window function is only reported once a row reaches it (as `INT-029` / `INT-030`); a query where no row matches returns an empty result instead. An unknown predicate in an `or` branch is reported only when a row reaches it, and one in a `not` / `not-join` body makes the body match nothing, so the `not` keeps every outer row.
<!-- @end -->

This means queries referencing UDFs can be parsed and stored before the UDF is registered, and will succeed once registration occurs.
<!-- @since v3.0.0 -->

### Functions That Can Fail

`Minigraf::try_register_predicate(name, f)` and `Minigraf::try_register_aggregate(name, init, step, finalise)` take closures that return a `Result`, with `UdfError` as the error type:

```rust
use minigraf::{UdfError, Value};

db.try_register_predicate("email?", |v| match v {
    Value::String(s) => Ok(s.contains('@')),
    _ => Err(UdfError::new("not a string")),
})?;
```

A call that returns `Err` fails the whole query with `QRY-014`, ``user function `email?` failed: not a string``. It is never read as `false` or `nil`, and no later call of any user function is made for that query. A cursor treats it as any other read error: it returns the rows of the batches the query finished before the failure, then the error, then no more rows. Functions registered with `register_predicate` / `register_aggregate` never fail this way.

### Registration Snapshots and Re-Entry

A query reads its functions from the registry as it was when the query opened. A function registered later is not visible to a query or cursor that is already open; every query opened afterwards sees it.

No database lock is held while a user function runs, so a function may call back into its own database:

| From inside a user function | Outer query is a plain read | Outer query runs in `WriteTransaction::execute` |
|---|---|---|
| `execute` (query), `query`, `prepare`, `PreparedQuery::execute` / `query`, `fact_log` | Runs, on its own snapshot | `INT-001` |
| `execute` (`transact` / `retract` / `rule`), `begin_write` | Runs; the outer query does not see the write | `INT-001` |
| `checkpoint`, `verify`, `rebuild_indexes` | Runs | `INT-001` |
| `register_predicate` / `register_aggregate` and their `try_` forms | Runs; the outer query does not see the new function | Runs |
| A nested query that calls another user function | Runs | `INT-001` (the nested query is refused) |

Before v3.0.0 a user function that registered a function, or that ran a checkpoint inside a write transaction, deadlocked.

### In the Browser

`BrowserDb.registerPredicate(name, fn)` and `BrowserDb.registerAggregate(name, { init, step, finish })` register JavaScript functions, synchronously. Each value arrives as a `{ type, value }` object: `type` is `'string'`, `'integer'` (`value` a `bigint`), `'float'`, `'boolean'`, `'ref'` (an entity UUID string), `'keyword'`, `'null'` (no `value`) or `'set'` (`value` an array of these). A predicate returns a boolean. `init()` returns an accumulator, `step(acc, value)` returns the next one, and `finish(acc, count)` returns the result as a plain value or a `{ type, value }` form, as a prepared query's binding takes it. The three are called with no `this`.

```javascript
db.registerPredicate('adult?', (v) => v.type === 'integer' && v.value >= 18n);
db.registerAggregate('product', {
  init: () => 1n,
  step: (acc, v) => acc * v.value,
  finish: (acc) => ({ type: 'integer', value: acc }),
});
```

A function that throws fails the query with `QRY-014` and the error's `message` (else `String(e)`), and so does a predicate that returns anything but a boolean, or a `finish` result that is not a value. From inside a function, `query`, `prepare` and the `register*` methods run at once; `execute`, `checkpoint` and `importGraph` return a promise that starts only after the outer query has returned. See [the browser section](use-cases#user-functions) of the use cases.

### In the other bindings

Every binding registers predicates and aggregates written in its own language. A predicate is called as `[(name ?v)]`, and an aggregate is used as `(name ?v)` in `:find` or with `:over`; each group or window partition gets its own accumulator.

```python
class Adult(minigraf.Predicate):          # Python
    def test(self, value):
        return isinstance(value, minigraf.Value.INT64) and value.value >= 18

db.register_predicate("adult?", Adult())
db.register_aggregate("product", Product())  # Product.accumulator() returns a minigraf.Accumulator
```

```kotlin
object Adult : MiniGrafPredicate {           // Kotlin (JVM and Android); Java implements the same interfaces
    override fun test(value: MiniGrafValue) = value is MiniGrafValue.Int64 && value.value >= 18
}
db.registerPredicate("adult?", Adult)
db.registerAggregate("product", Product)   // MiniGrafAggregate.accumulator() returns a MiniGrafAccumulator
```

```swift
final class Adult: MiniGrafPredicate {       // Swift: the protocols are AnyObject, Sendable
    func test(value: MiniGrafValue) throws -> Bool {
        if case .int64(let age) = value { return age >= 18 }
        return false
    }
}
try db.registerPredicate(name: "adult?", predicate: Adult())
```

```js
db.registerPredicate('adult?', (v) => v.type === 'integer' && v.value >= 18n)   // Node.js
db.registerAggregate('product', { init: () => 1n, step: (acc, v) => acc * v.value, finish: (acc) => acc })
```

```c
/* C: SQLite-style function pointer, user_data and destructor; values as JSON */
minigraf_register_predicate(db, "adult?", adult, NULL, NULL);
minigraf_register_aggregate(db, "product", product_new, product_step, product_finish,
                            free, NULL, NULL);
```

- **Values.** Python, Java, Android and Swift pass a `MiniGrafValue` (`minigraf.Value` in Python) and `finish` returns one. Node.js passes a `FactValue` (`{ type, value }`, as in the browser) and reads `finish`'s result as a bind value. C passes the value as JSON text (`{"type":"integer","value":30}`) and takes the result through `minigraf_udf_result`.
- **Failures.** A callback that throws (or, in C, returns -1) fails the query with `QRY-014`, raised as `MiniGrafError.Query` in the UniFFI bindings. Throw `MiniGrafUdfError.Failed(msg)` (`minigraf.UdfError.Failed` in Python) to choose the message; any other exception fails the call too, with its text as the message. Java, whose lambdas cannot throw that checked Kotlin exception, throws an unchecked one. Registering a name twice is `INT-040`; predicates and aggregates have separate names.
- **Lifecycle.** The database keeps each function until it and every cursor and prepared query opened from it are released. There is no unregister. In C, `destroy(user_data)` runs then, and `acc_free(acc)` runs when the query drops an accumulator. A query sees the functions registered when it opened.
- **Threading.** A callback runs on the thread that calls `execute` or a cursor's `next_batch`. In Python, Java, Android, Swift and C, queries on different threads may call the same object at once, so it must be thread-safe; Python's callback re-acquires the GIL. In Node.js a function runs on the JavaScript thread that registered it.
- **Re-entry.** A callback may query its own database, open a cursor, write or register another function, with the [rules above](#registration-snapshots-and-re-entry); no binding lock is held while the core runs. Each call crosses the FFI once, so a function over many rows costs one call per value.

Each binding's README has the full example and its language's details.
<!-- @end -->

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

<!-- @since v2.0.11 -->
`=` and `!=` compare as joins do: a keyword equals a ref to the entity it names, so
`[(= ?friend :bob)]` keeps a row whose `?friend` is a ref to `:bob`'s entity, as the
pattern `[?x :friend :bob]` matches it ([#744](https://github.com/project-minigraf/minigraf/issues/744)).
A keyword still equals only the same keyword, and `<`, `>`, `<=` and `>=` on a keyword
or a ref keep no row.

<!-- @end -->

<!-- @since v3.0.0 -->
A filter is a list inside a vector. A bare list in `:where` is a rule call, and a rule's
name is a symbol that is neither a variable nor an operator, so `(< ?age 30)` without
the vector fails with `PRS-097`.

<!-- @end -->

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

<!-- @since v3.0.0 -->
When another clause also binds the output variable, the binding is a test: the row is kept only if the result equals the variable's value (floats by IEEE `==`, so `0.0` equals `-0.0`; an integer never equals a float; NaN equals nothing), whatever order the planner runs the clauses in.

```datalog
;; [:a :age 30] [:b :age 40] [:p1 :score 41]
[:find ?a ?x :where [?e :age ?a] [:p1 :score ?x] [(+ ?a 1) ?x]]
;; => [40 41]

[(+ ?v 1) ?v]                     ;; never matches
[(* ?a 2) ?x] [(+ ?a ?a) ?x]      ;; both bind ?x: they must agree
```
<!-- @end -->

<!-- @until v3.0.0 -->
An expression binding replaces the variable's value where the planner runs it, even when another clause also binds it.
<!-- @end -->

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

<!-- @since v3.0.0 -->
**Set predicates** (return `Boolean`): `in?` `not-in?` `any-in?` `all-in?` `none-in?` ([Sets](#sets))

<!-- @end -->

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

<!-- @since v3.0.0 -->
## Sets

### Set values

A set literal `#{...}` holds scalar literals: strings, numbers, booleans, `nil`, keywords
and `#uuid` refs. It is a value like any other: transact it, retract it, match it in a
pattern, compare it, join on it, group by it.

```datalog
(transact [[:alice :user/scopes #{:scope/read :scope/write}]
           [:bob   :user/scopes #{:scope/read}]])

(query [:find ?e :where [?e :user/scopes #{:scope/write :scope/read}]])
;; => alice — element order does not matter
```

- Duplicates are removed: `#{1 1 2}` is `#{1 2}`.
- Two elements are the same when they are equal values of the same type: `0.0` and
  `-0.0` are one element (`-0.0` is kept), every NaN is one element, `1` is not `1.0`,
  and a keyword is not a ref to the entity it names.
- A set cannot hold a set (`PRS-119`), and its elements are literals only: a variable,
  bind slot, vector, list or map inside is `PRS-120`. An unclosed `#{` is `PRS-121`.
- A stored set may take up to 4,068 bytes, counting each keyword element as 6 bytes and
  each ref as 10 (`WAL-003`). Sets in query text and bind values have no limit.
- Results show a set as `#{...}` in the REPL and as a JSON array in JSON output, in a
  fixed canonical order.

### Set predicates

| Predicate | Operands | True when |
|---|---|---|
| `(in? x S)` | value, set | `x` is an element of `S` |
| `(not-in? x S)` | value, set | `x` is not an element of `S` |
| `(any-in? A B)` | set, set | the sets share an element |
| `(all-in? A B)` | set, set | every element of `A` is in `B` |
| `(none-in? A B)` | set, set | the sets share no element |

```datalog
[(in? ?role #{:role/admin :role/owner})]          ;; filter
[(any-in? ?scopes #{:scope/write}) ?can-write]    ;; binds true or false
[(all-in? ?required ?granted)]                    ;; both operands variables
```

An operand can be a literal set, a set-valued attribute's variable or a bind slot.
Membership uses the element equality above. `in?` of an empty set is false and
`not-in?` true; `all-in?` and `none-in?` of an empty first set are true and `any-in?`
false. A set where a value is needed, or a value where a set is needed, is a type
mismatch: a filter drops the row and a binding binds nothing, so `not-in?` with a
second operand that is no set is never true. `contains?` stays a string predicate.
The five names cannot be registered as functions.

### `collect-set`

`[(collect-set ?e :attr) ?s]` binds, for each row, the set of the values of
`[?e :attr ?v]`: the values of a multi-valued attribute gathered into one set.

```datalog
(transact [[:alice :user/scope :scope/read] [:alice :user/scope :scope/write]
           [:bob   :user/scope :scope/read]])

(query [:find ?e
        :where [?e :user/role _]
               [(collect-set ?e :user/scope) ?s]
               [(in? :scope/write ?s)]])
;; => alice
```

- It reads the facts the pattern `[?e :attr ?v]` would match under the query's
  `:as-of` and `:valid-at`, retractions applied; it reads stored facts only, never a
  rule's derived ones.
- An entity with no value gives `#{}`, and the row is kept.
- `?e` must be bound by an earlier clause (`INT-018`); a keyword names its entity. The
  attribute is a keyword literal.
- It works in `:where`, rule bodies, `not-join` bodies and `or` branches. When `?s` is
  already bound, it keeps the rows where the two sets are equal.
- Missing output variable: `PRS-122`. Operands that are not a variable then an attribute
  keyword: `PRS-123`. A value found that is itself a set: `QRY-015` (match a set-valued
  attribute with a pattern instead). Inside another expression: `PRS-067`.
- The set counts toward `query_memory_limit` (`QRY-012`).

---
<!-- @end -->

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

<!-- @since v3.0.0 -->
A rule head is the rule's name followed by variables. A name that is a variable or an
operator fails with `PRS-105`, and a head argument that is not a variable, such as
`(rule [(r ?a :c) …])`, with `PRS-114`; bind the variable in the body instead,
`(rule [(r ?a ?x) [?a :edge ?x] [(= ?x :c)]])`.

The head's first argument is the entity of the fact the rule derives: it must bind a ref
or a keyword (which names an entity), or the query fails with `QRY-013`; the second
argument may bind any value. A head variable the body does not bind fails with `INT-022`.
Both hold for rules with `not`, `not-join`, `or` or `or-join` too, which before v3.0.0
dropped such rows silently. An `or` branch in a rule body can use predicates registered
with `register_predicate`, as the rest of the body can.

<!-- @end -->

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

Parse and validate a query once; execute it many times with different bind values — including temporal filters — without re-parsing it on each call. Each execution substitutes the values and plans the query with them.

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

<!-- @until v3.0.0 -->
| Position | Permitted `BindValue` variants |
|---|---|
| Entity in pattern (`[$entity :attr ?v]`) | `Entity(Uuid)` |
| Value in pattern (`[?e :attr $val]`) | `Val(Value)` |
| `:as-of $tx` | `TxCount(u64)`, `Timestamp(i64)` |
| `:valid-at $date` | `Timestamp(i64)`, `AnyValidTime` |
<!-- @end -->
<!-- @since v3.0.0 -->
| Position | Permitted `BindValue` variants |
|---|---|
| Entity in pattern (`[$entity :attr ?v]`) | `Entity(Uuid)`, `Val(Value::Ref(Uuid))` |
| Value in pattern (`[?e :attr $val]`), expression, rule argument | `Val(Value)`, `Entity(Uuid)` (as `Value::Ref`) |
| `:as-of $tx` | `TxCount(u64)`, `Timestamp(i64)` |
| `:valid-at $date` | `Timestamp(i64)`, `AnyValidTime` |

A missing bind value is `API-022`, a bind slot in an attribute position `API-023` (from `prepare`), and a bind value of the wrong kind for its slot `API-024`. Before v3.0.0 these were `INT-033`, `INT-034` and `INT-035`.
<!-- @end -->

**Attribute position is not parameterisable** — the attribute decides which index a pattern reads, so it must be known when the query is prepared.

### API

```rust
use minigraf::{Minigraf, OpenOptions, BindValue};

let db = OpenOptions::new().path("agents.graph").open()?;

// Prepare once — parses and validates the query
let pq = db.prepare(
    "(query [:find ?belief
             :as-of $tx
             :where [$entity :belief/value ?belief]])"
)?;

// Execute many times — the bind values are substituted and the query runs
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
- In the browser, `BrowserDb.prepare(text)` returns a `BrowserPreparedQuery` with `execute(bindings)` (JSON, like `execute`) and `query(bindings)` (a `BrowserCursor`). Bindings are an object of slot name to a boolean, a `number` (an integer when `Number.isInteger`, else a float), a `bigint`, a string, `null`, or a `{ type, value }` form for the other kinds: `{ type: 'float', value: 2 }`, `'keyword'`, `'ref'`, `'entity'`, `'txCount'`, `'timestamp'` and `{ type: 'anyValidTime' }`. See [Tutorial 8](tutorial-08-prepared-queries#in-the-browser).
<!-- @end -->
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
- **Prepared statements** (`db.prepare(str)` → `PreparedQuery`) — parse once, execute many times with named `$slot` bind values — see [Prepared Statements](#prepared-statements-rust-api) above.
