---
title: Graph Traversal Patterns
nav: Graph traversal
section: Cookbook
order: 1
---
Recipes for navigating graph-structured data. Each recipe is self-contained: re-define any
rules it needs even if they appear elsewhere on this page.

← [Home](home) | [Time-Travel Idioms →](cookbook-time-travel)

---

## Recipe 1 — Direct neighbors (1-hop)

**Problem:** Find all entities directly connected to a given node via a specific edge attribute.

```datalog
;; Org chart: direct reports of :ceo
(transact [[:alice :reports-to :ceo]
           [:bob   :reports-to :ceo]
           [:carol :reports-to :alice]])

(query [:find ?person
        :where [?person :reports-to :ceo]])
;; => :alice, :bob
```

**Notes:**
- Join with `:person/name` to project human-readable names: add `[?person :person/name ?name]` and include `?name` in `:find`
- Works for any edge attribute: `:follows`, `:depends-on`, `:connected`, `:linked-to`
- Reverse direction — who does `:alice` report to: `[:alice :reports-to ?manager]`

---

## Recipe 2 — Transitive closure (all reachable nodes)

**Problem:** Find all nodes reachable from a root by following an edge attribute recursively.

```datalog
;; All reports under :ceo (direct and indirect)
(transact [[:alice :reports-to :ceo]
           [:bob   :reports-to :ceo]
           [:carol :reports-to :alice]
           [:dave  :reports-to :carol]])

(rule [(all-reports ?manager ?report)
       [?report :reports-to ?manager]])
(rule [(all-reports ?manager ?report)
       [?mid :reports-to ?manager]
       (all-reports ?mid ?report)])

(query [:find ?report
        :where (all-reports :ceo ?report)])
;; => :alice, :bob, :carol, :dave
```

**Notes:**
- Minigraf uses semi-naive fixed-point evaluation; terminates correctly on graphs with cycles
- The base case catches direct edges; the recursive case handles any depth
- Rule names are session-global; use descriptive names to avoid collisions across queries in the same session

---

## Recipe 3 — Reachability check

**Problem:** Test whether a path exists between two specific nodes.

```datalog
;; Social graph
(transact [[:alice :follows :bob]
           [:bob   :follows :carol]
           [:carol :follows :dave]])

(rule [(reachable ?a ?b) [?a :follows ?b]])
(rule [(reachable ?a ?b) [?a :follows ?m] (reachable ?m ?b)])

;; Non-empty result = :alice can reach :dave; empty = cannot
(query [:find ?dst
        :where (reachable :alice ?dst)
               [(= ?dst :dave)]])
;; => :dave
```

**Notes:**
- Only presence or absence of results matters; the returned value is the destination itself
- To count all reachable nodes from a root: `(count ?dst)` — returns no results (empty set) if unreachable
- Substitute both entity arguments with variables to check reachability across all node pairs

---

## Recipe 4 — Leaf nodes (no outgoing edges)

**Problem:** Find nodes that have no outgoing edges of a given type.

```datalog
;; Org chart: employees with no direct reports
(transact [[:alice :reports-to :ceo]
           [:bob   :reports-to :ceo]
           [:carol :reports-to :alice]])

(query [:find ?person
        :where [?person :reports-to _]
               (not-join [?person] [_ :reports-to ?person])])
;; => :bob, :carol (nobody reports to them)
```

**Notes:**
- `not-join [?person]` declares `?person` as the shared variable; the inner pattern checks for the absence of incoming edges
- For "nodes with no outgoing edges" (sources): flip to `(not-join [?person] [?person :reports-to _])`
- Works equally well for dependency graphs, follower graphs, and any directed edge attribute

---

## Recipe 5 — Common ancestors of two nodes

**Problem:** Find all nodes that are ancestors of both a given pair of nodes.

```datalog
;; Org chart: find shared managers of :carol and :dave
(transact [[:alice :reports-to :vp]
           [:bob   :reports-to :vp]
           [:carol :reports-to :alice]
           [:dave  :reports-to :bob]
           [:vp    :reports-to :ceo]])

(rule [(ancestor ?person ?anc) [?person :reports-to ?anc]])
(rule [(ancestor ?person ?anc) [?person :reports-to ?mid] (ancestor ?mid ?anc)])

(query [:find ?common
        :where (ancestor :carol ?common)
               (ancestor :dave ?common)])
;; => :vp, :ceo (shared ancestors of both)
```

**Notes:**
- Result includes all shared ancestors, not just the nearest common ancestor (LCA)
- To find only the LCA: add `(not-join [?common] (ancestor :carol ?closer) (ancestor :dave ?closer) (ancestor ?closer ?common))` — complex but expressible
- The two `ancestor` sub-queries are independent; Minigraf evaluates each and intersects

---

## Recipe 6 — Descendants of multiple roots

**Problem:** Find all nodes reachable from any node in a set of starting points.

```datalog
;; All reports under either :alice or :bob (union of their subtrees)
(transact [[:carol :reports-to :alice]
           [:dave  :reports-to :alice]
           [:eve   :reports-to :bob]])

(rule [(all-reports ?manager ?report)
       [?report :reports-to ?manager]])
(rule [(all-reports ?manager ?report)
       [?mid :reports-to ?manager]
       (all-reports ?mid ?report)])

(query [:find ?report
        :where (or
                 (all-reports :alice ?report)
                 (all-reports :bob ?report))])
;; => :carol, :dave (under :alice) and :eve (under :bob)
```

**Notes:**
- `(or ...)` unions the results of both branches; `?report` is the shared output variable introduced by either branch
- Each branch is a complete sub-query; results are deduplicated across branches
- Equivalent to two separate queries + set union, but resolved in a single round-trip

---

## Recipe 7 — Neighbor count (out-degree)

**Problem:** Count how many outgoing edges each node has.

```datalog
;; Social graph: how many people does each user follow?
(transact [[:alice :follows :bob]
           [:alice :follows :carol]
           [:bob   :follows :carol]])

(query [:find ?person (count ?followed)
        :with ?followed
        :where [?person :follows ?followed]])
;; => [(:alice, 2), (:bob, 1)]
```

**Notes:**
- `:with ?followed` keeps each followed-entity row distinct before counting; without it, duplicate `(?person, ?followed)` pairs would collapse before the aggregate
- For in-degree (how many edges point *to* each node): `[?follower :follows ?person]`, count `?follower`
- Use `count-distinct ?followed` if your model can have duplicate edges between the same pair

---

## Recipe 8 — Type-filtered traversal

**Problem:** Traverse edges of a specific type, stopping at nodes that match a predicate.

```datalog
;; Call graph: find all core functions reachable from :main — skip I/O layer nodes
(transact [[:main    :calls :init]
           [:main    :calls :process]
           [:process :calls :save]
           [:save    :layer :io]
           [:init    :layer :core]
           [:process :layer :core]])

(rule [(core-reachable ?from ?to)
       [?from :calls ?to]
       (not [?to :layer :io])])
(rule [(core-reachable ?from ?to)
       [?from :calls ?mid]
       (not [?mid :layer :io])
       (core-reachable ?mid ?to)])

(query [:find ?fn
        :where (core-reachable :main ?fn)])
;; => :init, :process  (not :save, which is :io)
```

**Notes:**
- The `not` on `?to` excludes io-layer destinations from results; the `not` on `?mid` prevents traversal *through* io-layer nodes
- Replace `(not [?to :layer :io])` with any predicate: `[(= ?depth 0)]`, `(not [?node :status :deprecated])`, etc.
- Combine with `:as-of` to traverse the graph as it existed at a past snapshot

---

## Recipe 9 — Edge reification (property graphs)

**Problem:** Attach properties (weight, label, timestamp) to individual edges.

```datalog
;; Model weighted, labelled edges as first-class entities
(transact [[:edge-ab :edge/from   :alice]
           [:edge-ab :edge/to     :bob]
           [:edge-ab :edge/label  :knows]
           [:edge-ab :edge/weight 5]
           [:edge-ab :edge/since  "2020-01-01"]
           [:edge-ac :edge/from   :alice]
           [:edge-ac :edge/to     :carol]
           [:edge-ac :edge/label  :knows]
           [:edge-ac :edge/weight 2]])

;; Find Alice's connections with weight ≥ 3
(query [:find ?neighbor ?weight
        :where [?edge :edge/from   :alice]
               [?edge :edge/to     ?neighbor]
               [?edge :edge/weight ?weight]
               [(>= ?weight 3)]])
;; => [:bob 5]

;; Traverse reified edges recursively
(rule [(prop-reachable ?from ?to)
       [?e :edge/from ?from]
       [?e :edge/to   ?to]
       [?e :edge/label :knows]])
(rule [(prop-reachable ?from ?to)
       [?e  :edge/from  ?from]
       [?e  :edge/to    ?mid]
       [?e  :edge/label :knows]
       (prop-reachable ?mid ?to)])

(query [:find ?node
        :where (prop-reachable :alice ?node)])
;; => :bob, :carol (and deeper if they have :knows edges)
```

**Notes:**
- The edge entity (`:edge-ab`) is a stable identifier; its properties can be updated over time with full bitemporal history — e.g., "what was the weight of this connection in 2023?"
- Reification trades simplicity (direct `:follows` triples) for expressiveness (queryable edge metadata)
- Each edge entity can carry an arbitrary number of attributes; they are all queryable and indexable

---

← [Home](home) | [Time-Travel Idioms →](cookbook-time-travel)

Reference: [Recursive rules](datalog-reference#recursive-rules), [Negation](datalog-reference#negation), [Aggregation](datalog-reference#aggregation)
