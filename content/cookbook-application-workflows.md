---
title: Application Workflow Patterns
nav: App workflows
section: Cookbook
order: 4
---
End-to-end recipes for common Minigraf deployment scenarios. Rust API snippets appear
where the native API is required; all other recipes use the Datalog REPL syntax.

← [Bitemporal Modeling](cookbook-bitemporal-modeling) | [Home →](home)

---

## Agent memory

---

### Recipe 1 — Store, update, and retract beliefs

**Problem:** Implement an agent belief lifecycle — assert, correct, and retract beliefs while preserving the full decision audit trail.

```datalog
;; Agent records a belief
(transact [[:agent-session-1 :belief/sky-color "blue"]])  ;; tx 1

;; Belief is corrected
(retract [[:agent-session-1 :belief/sky-color "blue"]])
(transact [[:agent-session-1 :belief/sky-color "red"]])   ;; tx 3

;; Current belief
(query [:find ?color
        :where [:agent-session-1 :belief/sky-color ?color]])
;; => "red"

;; What did the agent believe before the correction?
(query [:find ?color
        :as-of 1
        :valid-at :any-valid-time
        :where [:agent-session-1 :belief/sky-color ?color]])
;; => "blue"
```

**Notes:**
- Assert multiple beliefs in a single transaction: `[[:agent :belief/A "x"] [:agent :belief/B "y"]]` — all receive the same tx-count
- Store the tx-count alongside decisions: `(transact [[:decision-42 :decision/belief-tx 1]])` — use it later as the `:as-of` handle for audits
- One `.graph` file per agent instance; each agent's memory is private (see Recipe 9)

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IFtbOmFnZW50LXNlc3Npb24tMSA6YmVsaWVmL3NreS1jb2xvciAiYmx1ZSJdXSkKKHJldHJhY3QgW1s6YWdlbnQtc2Vzc2lvbi0xIDpiZWxpZWYvc2t5LWNvbG9yICJibHVlIl1dKQoodHJhbnNhY3QgW1s6YWdlbnQtc2Vzc2lvbi0xIDpiZWxpZWYvc2t5LWNvbG9yICJyZWQiXV0p&title=Store%2C+update%2C+and+retract+beliefs&vt=any&e=:agent-session-1&view=map)

---

### Recipe 2 — Audit a past decision

**Problem:** Reconstruct exactly what an agent knew at the moment it made a specific decision.

```datalog
;; Store the tx-count when a decision was made
(transact [[:decision-42 :decision/made-at-tx 7]
           [:decision-42 :decision/choice     :route-A]])

;; Retrieve the tx-count for decision-42
(query [:find ?tx
        :where [:decision-42 :decision/made-at-tx ?tx]])
;; => 7

;; Reconstruct the agent's belief state at tx 7
(query [:find ?belief
        :as-of 7
        :valid-at :any-valid-time
        :where [:agent :belief/value ?belief]])
```

**Notes:**
- `:as-of 7` freezes the database to what was known after tx 7 — this is the agent's exact knowledge state at decision time
- Combine with `:valid-at` to also restrict which real-world facts were considered valid at that moment
- The tx-count stored in `:decision/made-at-tx` is the durable link between a decision record and its knowledge context

**Visualize:** [See a similar audit in the time travel visualizer: what an agent believed when it made a recommendation →](https://project-minigraf.github.io/minigraf-visualizer/#sample=agent-memory&tx=3&vt=any&e=:user-ana)

---

### Recipe 3 — High-frequency belief queries with prepared statements

**Problem:** Run the same query shape thousands of times per session with different entity IDs and tx snapshots without re-parsing each time.

```rust
use minigraf::{Minigraf, OpenOptions, BindValue};

let db = OpenOptions::new().open("agent.graph")?;

// Prepare once at session startup — parse and plan happen here
let pq = db.prepare(
    "(query [:find ?belief
             :as-of $tx
             :where [$entity :belief/value ?belief]])"
)?;

// Execute thousands of times — query plan is reused, only bind values differ
for (entity_id, tx) in agent_queries {
    let results = pq.execute(&[
        ("tx",     BindValue::TxCount(tx)),
        ("entity", BindValue::Entity(entity_id)),
    ])?;
    // process results...
}
```

**Notes:**
- `$tx` and `$entity` are named bind slots; `prepare()` parses and plans once; `execute()` substitutes values and runs
- `BindValue::TxCount(u64)` for `:as-of`-style tx bindings; `BindValue::Entity(uuid::Uuid)` for entity IDs; `BindValue::String(String)` for string values
- `PreparedQuery` is `Send + Sync`; safe to share across threads — each `execute()` call is independent

---

### Recipe 4 — GraphRAG pattern (architecture)

**Problem:** Combine a vector store for fuzzy retrieval with Minigraf for structured graph and bitemporal navigation.

The two layers serve complementary roles:

| Layer | Tool | Job |
|---|---|---|
| Fuzzy retrieval | Vector store (Chroma, Qdrant, Pinecone…) | Find entities similar to the query embedding |
| Graph + temporal backbone | Minigraf | Follow relationships, audit facts, rewind to past states |

**Pattern:**
1. Store the Minigraf entity UUID alongside its embedding in the vector store
2. On a query, retrieve top-k matching entity UUIDs from the vector store
3. Use those UUIDs as entry points into Minigraf for graph traversal and temporal queries

```datalog
;; Entry-point UUID retrieved from vector store
;; Follow relationships and fetch bitemporal metadata from Minigraf
(query [:find ?title ?approver ?approved-at
        :where [#uuid "550e8400-e29b-41d4-a716-446655440000" :doc/title      ?title]
               [#uuid "550e8400-e29b-41d4-a716-446655440000" :doc/approved-by ?approver-id]
               [?approver-id :person/name ?approver]
               [#uuid "550e8400-e29b-41d4-a716-446655440000" :doc/approved-at ?approved-at]])
```

**Notes:**
- Minigraf has no vector search — the vector store does fuzzy retrieval; Minigraf handles graph traversal and temporal queries
- Entity UUIDs are stable across time; they are the durable link between the two systems
- The vector store answers "which entities are relevant?"; Minigraf answers "what are their relationships and history?"

---

## Offline-first

---

### Recipe 5 — Record facts offline, correct on sync

**Problem:** Record a fact on a device offline, then retroactively correct it after the user reviews the data on sync.

```datalog
;; Device records a health measurement on 2025-06-01 (recorded offline)
(transact {:valid-from "2025-06-01"}
          [[:user :health/weight-kg 82.5]])  ;; tx 1

;; After sync, user notices a mis-entry and corrects it
(retract [[:user :health/weight-kg 82.5]])
(transact {:valid-from "2025-06-01"}
          [[:user :health/weight-kg 80.5]])  ;; tx 3 — corrected, same valid-from

;; Current value: corrected
(query [:find ?w :where [:user :health/weight-kg ?w]])
;; => 80.5

;; What was originally recorded (before correction)?
(query [:find ?w
        :as-of 1
        :valid-at :any-valid-time
        :where [:user :health/weight-kg ?w]])
;; => 82.5
```

**Notes:**
- The original measurement is preserved in tx history — the correction is auditable
- `valid-from "2025-06-01"` reflects when the measurement was *taken*, not when the device synced — this is valid-time modeling
- This pattern works identically whether the device was online or offline when recording; Minigraf does not distinguish

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IHs6dmFsaWQtZnJvbSAiMjAyNS0wNi0wMSJ9CiAgICAgICAgICBbWzp1c2VyIDpoZWFsdGgvd2VpZ2h0LWtnIDgyLjVdXSkKKHJldHJhY3QgW1s6dXNlciA6aGVhbHRoL3dlaWdodC1rZyA4Mi41XV0pCih0cmFuc2FjdCB7OnZhbGlkLWZyb20gIjIwMjUtMDYtMDEifQogICAgICAgICAgW1s6dXNlciA6aGVhbHRoL3dlaWdodC1rZyA4MC41XV0p&title=Record+facts+offline%2C+correct+on+sync&vt=any&e=:user&view=map)

---

### Recipe 6 — Detect what changed since last sync

**Problem:** Find all facts added after a known sync checkpoint so they can be pushed to a central store.

```datalog
;; Store the tx-count of the last successful sync
(transact [[:app/state :sync/last-tx-count 12]])

;; Retrieve it before each sync
(query [:find ?last-tx
        :where [:app/state :sync/last-tx-count ?last-tx]])
;; => 12

;; Find all weight facts recorded since tx 12
(query [:find ?value ?tx
        :valid-at :any-valid-time
        :where [:user :health/weight-kg ?value]
               [:user :db/tx-count      ?tx]
               [(> ?tx 12)]])
```

**Notes:**
- Store the last-sync tx-count as a fact; update it after each successful sync with `retract` + `transact`
- Replace the hard-coded `12` with the retrieved `?last-tx` value (or use a prepared query with a bind slot — see Recipe 3)
- To detect changes across multiple attributes, enumerate them with `or`: `(or [?e :health/weight-kg ?v] [?e :health/steps ?v])`

---

## Task and dependency graphs

---

### Recipe 7 — Model a task DAG

**Problem:** Model a project task graph and find all tasks that are transitively blocked by a given task.

```datalog
;; Task graph: tasks and their blocking relationships
(transact [[:task-a :task/name "Design API"]
           [:task-b :task/name "Implement endpoint"]
           [:task-c :task/name "Write tests"]
           [:task-d :task/name "Deploy"]
           [:task-a :task/blocks :task-b]
           [:task-b :task/blocks :task-c]
           [:task-b :task/blocks :task-d]])

;; Recursive rule: all tasks transitively blocked by a given task
(rule [(blocked-by ?blocker ?blocked)
       [?blocker :task/blocks ?blocked]])
(rule [(blocked-by ?blocker ?blocked)
       [?blocker :task/blocks ?mid]
       (blocked-by ?mid ?blocked)])

;; What does completing :task-a unblock (transitively)?
(query [:find ?name
        :where (blocked-by :task-a ?t)
               [?t :task/name ?name]])
;; => "Implement endpoint", "Write tests", "Deploy"
```

**Notes:**
- Model blocking as `:task/blocks` (A blocks B = B cannot start until A completes)
- The recursive rule finds all transitive dependents; Minigraf detects fixed points and terminates safely on cycles
- Add a `:task/status` attribute and filter with `(not [?t :task/status :complete])` to find only currently-blocked tasks

**Visualize:** [Open this recipe in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IFtbOnRhc2stYSA6dGFzay9uYW1lICJEZXNpZ24gQVBJIl0KICAgICAgICAgICBbOnRhc2stYiA6dGFzay9uYW1lICJJbXBsZW1lbnQgZW5kcG9pbnQiXQogICAgICAgICAgIFs6dGFzay1jIDp0YXNrL25hbWUgIldyaXRlIHRlc3RzIl0KICAgICAgICAgICBbOnRhc2stZCA6dGFzay9uYW1lICJEZXBsb3kiXQogICAgICAgICAgIFs6dGFzay1hIDp0YXNrL2Jsb2NrcyA6dGFzay1iXQogICAgICAgICAgIFs6dGFzay1iIDp0YXNrL2Jsb2NrcyA6dGFzay1jXQogICAgICAgICAgIFs6dGFzay1iIDp0YXNrL2Jsb2NrcyA6dGFzay1kXV0pCihydWxlIFsoYmxvY2tlZC1ieSA_YmxvY2tlciA_YmxvY2tlZCkKICAgICAgIFs_YmxvY2tlciA6dGFzay9ibG9ja3MgP2Jsb2NrZWRdXSkKKHJ1bGUgWyhibG9ja2VkLWJ5ID9ibG9ja2VyID9ibG9ja2VkKQogICAgICAgWz9ibG9ja2VyIDp0YXNrL2Jsb2NrcyA_bWlkXQogICAgICAgKGJsb2NrZWQtYnkgP21pZCA_YmxvY2tlZCldKQ&title=Task+DAG&vt=any)

---

### Recipe 8 — Query task DAG at a historical tx snapshot

**Problem:** Reconstruct which tasks were unblocked at a specific point in the project's history.

```datalog
;; At tx 5, :task-a was marked complete
;; What tasks became unblocked at that point that are not yet done?
(query [:find ?name
        :as-of 5
        :where [:task-a :task/status :complete]
               (blocked-by :task-a ?t)
               (not [?t :task/status :complete])
               [?t :task/name ?name]])
```

**Notes:**
- The `blocked-by` rule must be defined in the current REPL session before running this query — see Recipe 7 for the two-clause rule definition
- `:as-of 5` ensures the query sees the database state after tx 5, including the `:complete` status update
- The `blocked-by` rule is evaluated within the `:as-of 5` snapshot — only edges that existed at tx 5 are traversed
- Use this for retrospective sprint planning: "why was :task-d still blocked at the end of sprint 3?"

---

## Multi-tenant and fleet patterns

---

### Recipe 9 — One `.graph` file per agent or user (architecture)

**Problem:** Scale an agent fleet or multi-user application without a shared database.

**Pattern:** Each agent instance or user session maintains its own `.graph` file. There is no shared database.

```
agent-instance-1/  →  session-abc.graph
agent-instance-2/  →  session-def.graph
agent-instance-3/  →  session-ghi.graph
```

**When this is the right model:**
- Each agent handles one session, task, or conversation
- Memory is private and does not need to be shared across instances
- The `.graph` file travels with the agent (container volume, device storage, cloud object store)

```rust
// Each session gets its own file — no coordination needed
let db = OpenOptions::new().open(&format!("sessions/{session_id}.graph"))?;
```

**When to use a different approach:**
- Multiple agents need shared state → use a distributed database for that layer; Minigraf remains the per-agent L1 reasoning cache
- See the [Worker-local reasoning (L1 cache) pattern](use-cases#ai-agents) in Use-Cases for the fleet architecture

**Notes:**
- Each `.graph` file is completely self-contained; no server, no coordination, no network calls
- `OpenOptions::new().open(path)` creates the file if it doesn't exist, opens it otherwise
- `MiniGrafDb` is `Arc<Mutex<…>>` internally — the handle is `Send + Sync` but concurrent calls are serialized

---

← [Bitemporal Modeling](cookbook-bitemporal-modeling) | [Home →](home)

Reference: [Prepared queries](datalog-reference#prepared-statements-rust-api), [Bi-temporal queries](datalog-reference#bi-temporal-queries), [Use Cases](use-cases)
