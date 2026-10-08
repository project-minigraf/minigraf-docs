---
title: Use Cases
nav: Use cases
section: Reference
order: 40
---
Minigraf is designed for three primary deployment targets: AI agents, mobile apps, and WASM/browser. All three benefit from the same core properties — single-file, embedded, zero-configuration, bi-temporal.

**Support tiers.** The Rust crate and the Python binding are **Tier 1**: fully tested and released with every core release. Node.js, browser WASM, WASI, Java/JVM, Android, Swift and C are **Tier 2 (experimental)**: built and smoke-tested, released on a best-effort schedule. Tier 1 platforms are Linux (ext4, xfs), macOS (APFS) and Windows (NTFS) on local disk. See [support tiers](https://github.com/project-minigraf/minigraf/blob/main/PHILOSOPHY.md#support-tiers) and, before adopting, the [known issues in the current release](https://github.com/project-minigraf/minigraf/issues/421).

---

## AI Agents

Minigraf is a natural fit for agents that need **verifiable reasoning** — the ability to reconstruct exactly what the agent knew at the moment it made a decision, even after its beliefs have been updated or corrected.

Because every fact carries both a *transaction time* (when it was recorded) and a *valid time* (when it was true in the world), an agent's entire decision-making lineage is preserved and queryable:

```datalog
;; Agent records a belief
(transact [[:agent :belief/sky-color "blue"]])

;; Belief is later corrected
(retract [[:agent :belief/sky-color "blue"]])
(transact [[:agent :belief/sky-color "red"]])

;; Reconstruct what the agent believed at tx 1 — before the correction
(query [:find ?color :as-of 1 :where [:agent :belief/sky-color ?color]])
;; => "blue"
```

**Visualize:** [open this example at tx 1 in the time travel visualizer →](https://project-minigraf.github.io/minigraf-visualizer/#data=KHRyYW5zYWN0IFtbOmFnZW50IDpiZWxpZWYvc2t5LWNvbG9yICJibHVlIl1dKQoocmV0cmFjdCBbWzphZ2VudCA6YmVsaWVmL3NreS1jb2xvciAiYmx1ZSJdXSkKKHRyYW5zYWN0IFtbOmFnZW50IDpiZWxpZWYvc2t5LWNvbG9yICJyZWQiXV0p&title=Agent+belief+correction&tx=1&vt=any&e=:agent&view=map) Press → to step through the correction. For a larger example, see [the agent memory sample](https://project-minigraf.github.io/minigraf-visualizer/#sample=agent-memory&tx=3&vt=any&e=:user-ana): what an agent believed when it made a recommendation, and the correction that followed.

### Use cases

- **Agent memory with provenance** — Store what an agent believes, retract and correct without losing history, replay past states to audit decisions
- **Verifiable reasoning** — Post-hoc root cause analysis: rewind to the exact knowledge state at the moment of a mistake
- **Task planning graphs** — Model a DAG of sub-tasks as a graph; update dependencies over time; query historical task states with `:as-of`
- **Code dependency agents** — Embed call graphs or module dependency graphs; traverse with recursive Datalog rules
- **Multi-agent coordination** — Each agent carries its own `.graph` file as private embedded memory — no shared server, no network calls, no contention

### Why embedded is a feature, not a limitation

An agent's memory is private to that agent instance. Embedding Minigraf directly in the agent's process means no network latency, no external service to manage, offline-capable operation, and a portable `.graph` file that travels with the agent. The single-file model also makes agent memory trivially snapshotable, versioned, or rolled back.

### Scope: per-agent-instance memory, not a shared fleet brain

Minigraf is designed for *one agent instance, one `.graph` file*. This is a deliberate constraint.

In a distributed fleet where multiple agent nodes need to share a single memory store, Minigraf is the wrong tool — use a distributed database for that layer. But for the common case of an agent handling a session, a task, or a user interaction on a single machine, the single-file model is an advantage.

**Practical patterns for distributed deployments:**

- **Sticky sessions**: Route a given user or task ID consistently to the same node. The agent's local `.graph` stays coherent for the lifetime of that session.
- **Worker-local reasoning (L1 cache pattern)**: Use Minigraf for high-speed private reasoning during a task. At task completion, flush audited results to a central store. Minigraf handles the "internal monologue"; the central store handles global state.
- **Swarm / multi-agent**: Each agent carries its own `.graph` brain. Agents coordinate by passing small serialised sub-graphs to each other rather than sharing a database.

### A note on bitemporality and clock drift

Minigraf's transaction time is based on `tx_count` — a monotonic counter *per database instance*, not wall-clock time. Clock drift between machines does not affect the correctness of the bitemporal ordering within a single `.graph` file. The concern only arises if you attempt to merge two independently-operated instances, which Minigraf does not support.

### Prepared statements for the agentic memory loop

An agent running thousands of belief-state queries per session should not re-parse and re-plan the same query shape on every call. Prepared statements eliminate that overhead:

```rust
// Prepare once at session startup
let pq = db.prepare(
    "(query [:find ?belief
             :as-of $tx
             :where [$entity :belief/value ?belief]])"
)?;

// Execute thousands of times with different entity IDs and tx counts
for (entity_id, tx) in agent_queries {
    let beliefs = pq.execute(&[
        ("tx",     BindValue::TxCount(tx)),
        ("entity", BindValue::Entity(entity_id)),
    ])?;
    // process beliefs...
}
```

The query plan — join order, index selection, temporal filter structure — is computed once at `prepare()` time and reused on every `execute()`. Only the bind values differ. This is the `$slot` temporal parameterisation pattern: the same loop runs at any historical snapshot by varying the `TxCount` or `Timestamp` bind value.

### Pairing with vector stores (GraphRAG pattern)

Minigraf has no vector search — and doesn't need it. In a complete agentic memory stack, the two layers are complementary:

| Layer | Tool | Job |
|---|---|---|
| Fuzzy retrieval | Vector store (Chroma, Pinecone, etc.) | "Find things similar to this prompt" |
| Relational backbone | Minigraf | "Follow this relationship, audit this fact, rewind to this moment" |

The vector store holds embeddings alongside an entity UUID; that UUID is the entry point into Minigraf where the bitemporal history and relationships live. Vectors find the starting node; Minigraf navigates and audits from there.

```
Vector store:  embedding → entity_uuid
                                │
                                ▼
Minigraf:      entity_uuid ── :approved-by ──▶ approver
                    │
                    └── :approved-at "2025-01-14T14:00:00Z"
                    └── tx history (who recorded this, when)
```

---

## Mobile Apps

Minigraf is a natural fit for mobile applications that need to store and query relational or graph-structured data locally, without a network call.

### Why embedded is the right model for mobile

Mobile apps operate in environments where connectivity is intermittent and latency is unacceptable. Embedding Minigraf directly in the app process means queries are local, the `.graph` file travels with the app's data directory, and there is no server to provision or authenticate against. It is the same reason SQLite dominates mobile relational storage.

### Why bitemporality matters especially on mobile

Mobile data is inherently eventually consistent. A user records a fact offline, syncs later, and then discovers the fact was wrong — they need to correct it retroactively. A uni-temporal database forces you to delete and re-insert, losing the original record. Minigraf's bi-temporal model lets you retract the incorrect fact and assert the corrected one while preserving the full history:

```datalog
;; User logs a health measurement offline
(transact {:valid-from "2025-06-01"}
          [[:user :health/weight-kg 82.5]])

;; Later corrects a mis-entered value — old record preserved in history
(retract [[:user :health/weight-kg 82.5]])
(transact {:valid-from "2025-06-01"}
          [[:user :health/weight-kg 80.5]])

;; What did the app record before the correction?
(query [:find ?weight :as-of 1 :where [:user :health/weight-kg ?weight]])
;; => 82.5

;; What was actually true on 2025-06-01?
(query [:find ?weight
        :valid-at "2025-06-01"
        :where [:user :health/weight-kg ?weight]])
;; => 80.5
```

### Use cases

- **Health and fitness tracking** — Weight, nutrition, exercise logs with retroactive corrections
- **Personal knowledge management** — Notes, tags, and links stored as a graph; offline-first
- **Game state** — RPG character graphs, quest dependency DAGs, world state with rollback; bitemporality gives cheap save states
- **Local AI context** — On-device LLMs need structured facts to reason over; Minigraf as the relational backbone
- **Offline-first productivity apps** — Task managers, CRMs, project trackers where the device is the source of truth

### Integration

Minigraf ships native bindings for both mobile platforms via [UniFFI](https://github.com/mozilla/uniffi-rs). No Rust knowledge required — you use the `MiniGrafDb` class through idiomatic Kotlin or Swift.

| Platform | Artifact | Distribution |
|---|---|---|
| Android | `io.github.project-minigraf:minigraf-android` (`.aar`) | Maven Central |
| iOS | `MinigrafKit-<version>.xcframework.zip` | Swift Package Manager ([minigraf-swift](https://github.com/project-minigraf/minigraf-swift)) |

---

#### Android (Kotlin)

> **Support tier 2 (experimental).** Built and smoke-tested, released on a best-effort schedule, possibly after the core release. See [support tiers](https://github.com/project-minigraf/minigraf/blob/main/PHILOSOPHY.md#support-tiers).

##### 1 — Add the dependency

The AAR is published to Maven Central. In your module `build.gradle.kts`:

<!-- @until v3.0.0 -->
```kotlin
dependencies {
    implementation("io.github.project-minigraf:minigraf-android:2.0.4")
}
```
<!-- @end -->
<!-- @since v3.0.0 -->
```kotlin
dependencies {
    implementation("io.github.project-minigraf:minigraf-android:3.0.0")
}
```
<!-- @end -->

See [minigraf-android](https://github.com/project-minigraf/minigraf-android) for the current version.

##### 2 — Use in Kotlin

```kotlin
import uniffi.minigraf_ffi.MiniGrafDb
import uniffi.minigraf_ffi.MiniGrafError
import org.json.JSONObject

// File-backed database (persists to the app's data directory)
val db = MiniGrafDb.open(context.filesDir.absolutePath + "/myapp.graph")

// In-memory database (tests, ephemeral use)
val memDb = MiniGrafDb.openInMemory()

// Assert facts
db.execute("""
    (transact [[:alice :person/name "Alice"]
               [:alice :person/age 30]
               [:alice :friend :bob]
               [:bob   :person/name "Bob"]])
""")

// Query — returns a JSON string
val json = db.execute("""
    (query [:find ?name ?age
            :where [?e :person/name ?name]
                   [?e :person/age  ?age]])
""")
val result = JSONObject(json)
// result.getJSONArray("variables") == ["?name", "?age"]
// result.getJSONArray("results").getJSONArray(0).getString(0) == "Alice"

// Time travel — state as of transaction 1
val snapshot = db.execute(
    "(query [:find ?age :as-of 1 :where [:alice :person/age ?age]])"
)

// Flush dirty pages to disk
db.checkpoint()
```

##### Response shapes

| Command | JSON |
|---|---|
| `transact` | `{"transacted": <tx_count>}` |
| `retract` | `{"retracted": <tx_count>}` |
| `query` | `{"variables": [...], "results": [[...]]}` |
| `rule` | `{"ok": true}` |

##### Error handling

UniFFI generates a sealed class hierarchy extending `Exception`:

```kotlin
try {
    db.execute("(bad datalog)")
} catch (e: MiniGrafError.Parse) {
    Log.e("Minigraf", "Parse error: ${e.msg}")
} catch (e: MiniGrafError.Query) {
    Log.e("Minigraf", "Query error: ${e.msg}")
} catch (e: MiniGrafError.Storage) {
    Log.e("Minigraf", "Storage error: ${e.msg}")
}
```

##### Threading

`MiniGrafDb` is internally `Arc<Mutex<_>>` — safe to share, but each call blocks the calling thread. Wrap in `withContext(Dispatchers.IO)`:

```kotlin
val rows = withContext(Dispatchers.IO) {
    JSONObject(db.execute("(query [:find ?e :where [?e :type :event]])"))
        .getJSONArray("results")
}
```

---

#### iOS (Swift)

> **Support tier 2 (experimental).** Built and smoke-tested, released on a best-effort schedule, possibly after the core release. See [support tiers](https://github.com/project-minigraf/minigraf/blob/main/PHILOSOPHY.md#support-tiers).

##### 1 — Add the package

The Swift package lives in [minigraf-swift](https://github.com/project-minigraf/minigraf-swift).

<!-- @until v3.0.0 -->
In Xcode: **File → Add Package Dependencies**, enter `https://github.com/project-minigraf/minigraf-swift` and select **Up to Next Major Version** from `2.0.4`.

Or add it directly to your `Package.swift`:

```swift
// Package.swift
dependencies: [
    .package(url: "https://github.com/project-minigraf/minigraf-swift", from: "2.0.4"),
],
targets: [
    .target(
        name: "YourTarget",
        dependencies: [
            .product(name: "MinigrafKit", package: "minigraf-swift"),
        ]
    ),
],
```

Use 2.0.1 or later. Earlier semver tags in minigraf-swift point at a manifest with a placeholder checksum and do not resolve; see the [minigraf-swift README](https://github.com/project-minigraf/minigraf-swift).
<!-- @end -->
<!-- @since v3.0.0 -->
In Xcode: **File → Add Package Dependencies**, enter `https://github.com/project-minigraf/minigraf-swift` and select **Up to Next Major Version** from `3.0.0`.

Or add it directly to your `Package.swift`:

```swift
// Package.swift
dependencies: [
    .package(url: "https://github.com/project-minigraf/minigraf-swift", from: "3.0.0"),
],
targets: [
    .target(
        name: "YourTarget",
        dependencies: [
            .product(name: "MinigrafKit", package: "minigraf-swift"),
        ]
    ),
],
```
<!-- @end -->

##### 2 — Use in Swift

```swift
import MinigrafKit

// File-backed database (persists to the app's Documents directory)
let docsURL = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
let dbPath = docsURL.appendingPathComponent("myapp.graph").path
let db = try MiniGrafDb.open(path: dbPath)

// In-memory database (tests, ephemeral use)
let memDb = try MiniGrafDb.openInMemory()

// Assert facts
try db.execute(datalog: """
    (transact [[:alice :person/name "Alice"]
               [:alice :person/age 30]
               [:alice :friend :bob]
               [:bob   :person/name "Bob"]])
""")

// Query — returns a JSON string
let json = try db.execute(datalog: """
    (query [:find ?name ?age
            :where [?e :person/name ?name]
                   [?e :person/age  ?age]])
""")

if let data = json.data(using: .utf8),
   let result = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
   let rows = result["results"] as? [[Any]] {
    let name = rows[0][0] as? String  // "Alice"
    let age  = rows[0][1] as? Int     // 30
}

// Time travel — state as of transaction 1
let snapshot = try db.execute(
    datalog: "(query [:find ?age :as-of 1 :where [:alice :person/age ?age]])"
)

// Flush dirty pages to disk
try db.checkpoint()
```

##### Error handling

UniFFI generates a Swift `enum` conforming to `Error`:

```swift
do {
    try db.execute(datalog: "(bad datalog)")
} catch MiniGrafError.Parse(let msg) {
    print("Parse error: \(msg)")
} catch MiniGrafError.Query(let msg) {
    print("Query error: \(msg)")
} catch MiniGrafError.Storage(let msg) {
    print("Storage error: \(msg)")
}
```

##### Threading

`MiniGrafDb` calls block the calling thread. Dispatch to a background executor:

```swift
let json = try await Task.detached(priority: .utility) {
    try db.execute(datalog: "(query [:find ?e :where [?e :type :event]])")
}.value
```

### A note on sync

Minigraf does not provide built-in sync — this is intentional. The recommended pattern: use Minigraf for high-speed local reasoning; at sync points, export the facts you want to share and merge them into a central store using your application's conflict resolution logic. The bitemporal timestamps make conflict detection straightforward: compare `tx_count` values to determine which device recorded a fact first.

---

## WASM / Browser

Minigraf's single-file, zero-configuration design maps cleanly onto the browser environment. The Phase 8.1a WASM backend (complete as of v0.20.0) persists data in IndexedDB using page-granular records (one IndexedDB entry per 4KB page), giving browser applications a persistent, queryable graph database with no server required.

### Why a graph database in the browser

Most browser-side storage APIs (localStorage, IndexedDB raw) are key-value stores. They work for simple caching but are awkward for relational or graph-structured data — you end up maintaining foreign keys and join logic in JavaScript. Minigraf brings structured graph queries to the client, reducing the need for round-trips to a backend API.

### Why bitemporality matters for local-first web apps

Browser apps built on a local-first architecture face the same eventual-consistency problem as mobile: users make changes offline, and those changes may need to be corrected or reconciled later. The bitemporal model gives a principled way to record corrections without destroying history.

### Use cases

- **Offline PWAs** — A Progressive Web App that keeps its relational state in a `.graph` file in IndexedDB survives page reloads, browser restarts, and network outages
- **Privacy-sensitive applications** — Medical records, financial data, personal journals; process sensitive data entirely client-side, never leaves the browser
- **In-browser analytics and exploration** — Load a dataset into Minigraf in the browser and run Datalog queries; think Datasette for graph-structured data, no backend required
- **Developer tooling** — Dependency graphs, call graphs, module relationship maps queried client-side:

```datalog
;; Find all transitive dependencies of a module
(rule [(depends-on ?a ?b) [?a :module/depends-directly ?b]])
(rule [(depends-on ?a ?b) [?a :module/depends-directly ?m] (depends-on ?m ?b)])

(query [:find ?dep :where (depends-on :my-module ?dep)])
```

- **Collaborative local-first tools** — Each browser tab or user session carries its own `.graph` file as the local replica; application layer handles sync

### Integration (Phase 8)

> **Support tier 2 (experimental).** Built and smoke-tested, released on a best-effort schedule, possibly after the core release. See [support tiers](https://github.com/project-minigraf/minigraf/blob/main/PHILOSOPHY.md#support-tiers).

Two distinct WASM targets:

- **Browser (`wasm32-unknown-unknown`)**: Compiled and packaged with `wasm-pack`, which generates a `.wasm` binary, JavaScript glue code, and TypeScript `.d.ts` definitions. Public API annotated with `#[wasm_bindgen]`. Storage uses IndexedDB with page-granular records — only dirty pages are written on checkpoint. Published to npm as `@minigraf/browser`.

- **Server-side WASM (`wasm32-wasip1` / WASI)**: Standard `cargo build` to a WASI target; runs inside Wasmtime, Wasmer, Cloudflare Workers, and Fastly Compute. Complete as of Phase 8.1b (v0.20.0); more secure than Docker for agent sandboxing.

### Browser WASM — Phase 8.1a

Minigraf's browser build compiles to `wasm32-unknown-unknown` via `wasm-pack` and exposes a `BrowserDb` JavaScript/TypeScript class backed by IndexedDB. Data persists across page reloads.

#### Build

```bash
# Install wasm-pack if needed
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

wasm-pack build --target web --features browser
# Output: minigraf-wasm/ — contains minigraf_bg.wasm, minigraf.js, minigraf.d.ts
```

#### JavaScript API

```javascript
import init, { BrowserDb } from './minigraf-wasm/minigraf.js';
await init();

// Persistent database (IndexedDB-backed)
const db = await BrowserDb.open('my-graph');

// In-memory database (testing / ephemeral use)
const mem = BrowserDb.openInMemory();

// Execute Datalog — returns a JSON string
const result = JSON.parse(await db.execute(
  '(transact [[:alice :person/name "Alice"] [:alice :person/age 30]])'
));
// { "transacted": 1 }

const query = JSON.parse(await db.execute(
  '(query [:find ?name ?age :where [?e :person/name ?name] [?e :person/age ?age]])'
));
// { "variables": ["?name", "?age"], "results": [["Alice", 30]] }
```

#### Response shapes

| Command | JSON shape |
|---|---|
| `transact` | `{"transacted": <tx_id>}` |
| `retract` | `{"retracted": <tx_id>}` |
| `query` | `{"variables": [...], "results": [[...]]}` |
| `rule` | `{"ok": true}` |

<!-- @since v3.0.0 -->
#### Reading large results in batches

`query()` opens a `BrowserCursor` and returns it synchronously, for results you want to read a batch at a time. The answer is fixed when the cursor opens.

```javascript
const cursor = db.query('(query [:find ?name :where [?e :person/name ?name]])');
try {
  let batch;
  while ((batch = cursor.nextBatch(1000)) !== undefined) {
    for (const row of JSON.parse(batch)) console.log(row);
  }
} finally {
  cursor.close();
}
```

`cursor.vars()` returns the `:find` variables. Only queries are accepted: `transact`, `retract` and `rule` throw `API-012`, and a query with `$slot` bind slots throws `API-010`.

<!-- @end -->
#### Checkpoint and portability

`execute()` flushes dirty pages automatically. Call `checkpoint()` explicitly only after `importGraph()` or large bulk operations:

```javascript
await db.checkpoint();
```

Export the database as a portable `.graph` blob (byte-for-bit compatible with native files):

```javascript
const blob = db.exportGraph();           // Uint8Array
const file = new File([blob], 'db.graph');

// Import a native .graph file (must be checkpointed — no pending WAL)
const bytes = new Uint8Array(await file.arrayBuffer());
await db.importGraph(bytes);
```

#### Constraints

- **Browser only**: `BrowserDb` requires a browser environment with IndexedDB. Not compatible with Node.js, Deno, or Bun.
- **Single-threaded**: Runs on the main thread or in a Web Worker; no shared state between workers.
- **Install**: `npm install @minigraf/browser`

---

### Server-side WASM (WASI) — Phase 8.1b

WASI (WebAssembly System Interface) is a capability-based standard that gives WebAssembly programs access to OS-like primitives — stdin/stdout, clocks — without requiring a JavaScript engine. Minigraf compiles to `wasm32-wasip1` and runs in any WASI-compatible runtime.

#### Build

```bash
rustup target add wasm32-wasip1
cargo build --target wasm32-wasip1 --release --bin minigraf
# Output: target/wasm32-wasip1/release/minigraf.wasm
```

#### Run

```bash
# Wasmtime — pipe Datalog commands via stdin
echo '(transact [[:alice :name "Alice"]])' | \
  wasmtime run target/wasm32-wasip1/release/minigraf.wasm

# Wasmer
echo '(transact [[:alice :name "Alice"]])' | \
  wasmer run target/wasm32-wasip1/release/minigraf.wasm
```

The REPL reads commands from stdin and writes results to stdout, one result per line. All data is in-memory and discarded when the process exits.

> **Note:** The `--dir <path>` flag grants WASI filesystem access. The in-memory REPL does not read or write files, so `--dir` is not required. Add it only if you extend Minigraf with file-backed storage in a future phase.

#### Constraints

- **In-memory only**: WASI Phase 8.1b does not expose file-backed storage. Each invocation starts with an empty database.
- **Single-threaded**: WASI Preview 1 has no thread support. Concurrent access from multiple threads is not possible by design; `Mutex`/`RwLock` compile as single-threaded stubs.

#### Embedding in host applications

Minigraf can be embedded in any Wasmtime or Wasmer host. The host pipes Datalog commands to stdin and reads results from stdout.

**Wasmtime (Rust):**

```rust
use wasmtime::*;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};

let engine = Engine::default();
let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
preview1::add_to_linker_sync(&mut linker, |ctx| ctx)?;

let wasi = wasmtime_wasi::WasiCtxBuilder::new()
    .inherit_stdio()
    .build_p1();
let mut store = Store::new(&engine, wasi);

let module = Module::from_file(&engine, "minigraf.wasm")?;
let instance = linker.instantiate(&mut store, &module)?;
let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
start.call(&mut store, ())?;
```

**Wasmer and other runtimes:**

Each WASI runtime provides its own embedding API. Consult the runtime's documentation for the language of your choice:
- [Wasmtime embedding docs](https://docs.wasmtime.dev/lang.html) — Rust, Python, C, Go, .NET, Ruby
- [Wasmer embedding docs](https://docs.wasmer.io/sdk) — Rust, Python, JavaScript, Go, C/C++, PHP

The WASM binary produced by `cargo build --target wasm32-wasip1` follows the standard WASI Preview 1 ABI and works with any compliant host.

### Node.js

For Node.js consumers, the `@minigraf/wasi` npm package provides an ESM loader:

```sh
npm install @minigraf/wasi
```

```js
import { WASI } from "node:wasi";
import { startMinigrafWasi } from "@minigraf/wasi";

const wasi = new WASI({
  version: "preview1",
  args: ["minigraf"],
  env: process.env,
  preopens: { "/tmp": "/tmp" },
});

await startMinigrafWasi(wasi);
```

Use `MINIGRAF_WASI_WASM_PATH` or the `wasmPath` option to point the loader at an alternate `.wasm` file.

#### Cloudflare Workers / Fastly Compute

Both platforms support WASI. Build the binary as above and follow the platform's WASM deployment guide. Note that these platforms' WASI support is evolving — check the respective documentation for the latest compatibility status.

The `wasm` feature flag already gates `optimizer.rs` to keep the browser binary lean.

### Portability: export and import as a `.graph` file

Although the browser backend stores data as page-granular IndexedDB records internally, the `.graph` binary format is preserved for portability. A database created on desktop can be loaded in the browser, and vice versa. Export (download) reconstructs the file by reading pages in order; import (upload) writes each page to IndexedDB.

---

## Python

> **Support tier 1.** Fully tested and released with every core release. See [support tiers](https://github.com/project-minigraf/minigraf/blob/main/PHILOSOPHY.md#support-tiers).

Minigraf for Python ships as `minigraf` on PyPI with pre-built wheels for Linux x86_64/aarch64, macOS universal2, and Windows x86_64. No Rust toolchain required.

```sh
pip install minigraf
```

```python
from minigraf import MiniGrafDb

db = MiniGrafDb.open("myapp.graph")
db.execute('(transact [[:alice :person/name "Alice"] [:alice :person/age 30]])')
result = db.execute('(query [:find ?name :where [?e :person/name ?name]])')
# '{"variables":["?name"],"results":[["Alice"]]}'

db.checkpoint()
```

See the [minigraf-python README](https://github.com/project-minigraf/minigraf-python) for the full API reference.

---

## Node.js / TypeScript

> **Support tier 2 (experimental).** Built and smoke-tested, released on a best-effort schedule, possibly after the core release. See [support tiers](https://github.com/project-minigraf/minigraf/blob/main/PHILOSOPHY.md#support-tiers).

Minigraf for Node.js ships as `minigraf` on npm — a native addon (not WASM) with pre-built binaries. No build step required.

```sh
npm install minigraf
```

```typescript
import { MiniGrafDb } from 'minigraf';

const db = new MiniGrafDb('myapp.graph');
db.execute('(transact [[:alice :person/name "Alice"] [:alice :person/age 30]])');
const result = db.execute('(query [:find ?name :where [?e :person/name ?name]])');
// '{"variables":["?name"],"results":[["Alice"]]}'

db.checkpoint();
```

See the [minigraf-node README](https://github.com/project-minigraf/minigraf-node) for the full API reference.

---

## Java / JVM

> **Support tier 2 (experimental).** Built and smoke-tested, released on a best-effort schedule, possibly after the core release. See [support tiers](https://github.com/project-minigraf/minigraf/blob/main/PHILOSOPHY.md#support-tiers).

Minigraf for Java and Kotlin ships as `io.github.project-minigraf:minigraf-jvm` on Maven Central — a fat JAR with embedded native libraries. No Rust toolchain required.

<!-- @until v3.0.0 -->
```kotlin
// build.gradle.kts
dependencies {
    implementation("io.github.project-minigraf:minigraf-jvm:2.0.4")
}
```
<!-- @end -->
<!-- @since v3.0.0 -->
```kotlin
// build.gradle.kts
dependencies {
    implementation("io.github.project-minigraf:minigraf-jvm:3.0.0")
}
```
<!-- @end -->

```kotlin
import uniffi.minigraf_ffi.MiniGrafDb

val db = MiniGrafDb.open("/path/to/myapp.graph")
db.execute("""(transact [[:alice :person/name "Alice"]])""")
val result = db.execute("""(query [:find ?name :where [?e :person/name ?name]])""")
```

See the [minigraf-java README](https://github.com/project-minigraf/minigraf-java) for the full API reference and threading notes.

---

## C FFI

> **Support tier 2 (experimental).** Built and smoke-tested, released on a best-effort schedule, possibly after the core release. See [support tiers](https://github.com/project-minigraf/minigraf/blob/main/PHILOSOPHY.md#support-tiers).

Minigraf for C ships as platform tarballs on the minigraf-c GitHub Releases — a stable C header (`minigraf.h`) and prebuilt shared and static libraries. Suitable for any language with a C FFI.

Download from [minigraf-c releases](https://github.com/project-minigraf/minigraf-c/releases). Assets are named `minigraf-c-<version>-<platform>` for `linux-x86_64`, `linux-aarch64`, `macos-universal2` (`.tar.gz`) and `windows-x86_64` (`.zip`):

<!-- @until v3.0.0 -->
```sh
# Linux x86_64
curl -L https://github.com/project-minigraf/minigraf-c/releases/download/v2.0.4/minigraf-c-v2.0.4-linux-x86_64.tar.gz | tar xz
```
<!-- @end -->
<!-- @since v3.0.0 -->
```sh
# Linux x86_64
curl -L https://github.com/project-minigraf/minigraf-c/releases/download/v3.0.0/minigraf-c-v3.0.0-linux-x86_64.tar.gz | tar xz
```
<!-- @end -->

```c
#include "minigraf.h"

MiniGrafDb *db = minigraf_open("myapp.graph", NULL);
char *result = minigraf_execute(
    db, "(transact [[:alice :person/name \"Alice\"]])", NULL
);
minigraf_string_free(result);
minigraf_close(db);
```

Memory contract mirrors SQLite: `minigraf_execute` returns a heap-allocated string; call `minigraf_string_free` to release it. See the [minigraf-c README](https://github.com/project-minigraf/minigraf-c) for the full API.
