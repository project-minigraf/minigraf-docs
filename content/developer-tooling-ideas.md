---
title: Developer Tooling Ideas
nav: Tooling ideas
section: Reference
order: 45
---
This page collects ideas for developer tools that could be built on Minigraf. None of them exist as official tools. Each section describes the problem, why Minigraf fits, a fact schema to start from, and queries you can run in the REPL today. If you build one of these, or something like it, tell us in an [issue](https://github.com/project-minigraf/minigraf/issues).

For the main deployment targets (AI agents, mobile, browser), see [Use Cases](use-cases).

## What these ideas share

Most of these tools record facts about code, commits or runs, and learn more about them later. The two time axes map to that directly:

- **Valid time** is when something was true in the code or in the world: a commit, a release, the minute of an incident.
- **Transaction time** is when the tool learned it.

The gap between the two is where these tools find their value. A vulnerability is published months after a release ships. A bisect finds the bad commit weeks after it merged. A deploy log arrives after the alert it explains. A store that keeps only one time axis loses either the history of the code or the history of what you knew about it.

Each example below runs in a fresh REPL session, in order. Transaction numbers in `:as-of` count from 1 in that session.

<!-- @until v3.0.0 -->
> **v2.x:** two values of one entity and attribute written in one `transact` read back as one value ([#371](https://github.com/project-minigraf/minigraf/issues/371)). The examples on this page never do that, so they run on every version. Keep it in mind when you extend them.
<!-- @end -->

## Git sidecar

A bi-temporal sidecar for git repositories. It sits beside git, keyed by commit hash, and adds what git lacks: history by code entity instead of by line, facts learned after a commit, a durable record of branch moves, and help with merge conflicts. Several other ideas on this page attach their facts to the same commit and entity index.

**Why a sidecar, not a new version control system.** Git's strength is its network: hosts, CI, editors and habits. Technically strong alternatives such as Pijul and Fossil stayed niche. Jujutsu is gaining ground because it uses git as its storage. A sidecar needs no new workflow and works with any git host.

**Git's real blind spots.** Git is not simply "one time axis". Its history is a graph of snapshots, and each commit has an author date and a committer date. The gaps are elsewhere:

- **The history of the history is lost.** Branches are pointers that move. After a force-push or a rebase, the old state of a branch is gone from the shared repository. The reflog keeps it on one machine only, and it expires.
- **Facts learned later have no good home.** "This commit introduced bug 812" is learned weeks after the commit. Git notes can hold it, but they are rarely pushed and have no history of their own.
- **Git tracks lines, not things.** It has no idea what a function is. It guesses renames and moves.
- **Search is slow and text-only.** `git log -S`, `git log -L`, `--grep` and `blame` scan history on every query, cannot be combined, and lose a function that moves to another file.

### Conflict help

This is the pain developers feel every day. A sidecar cannot change how git merges, but it can plug in through merge drivers, `git mergetool` and hooks, and supply the knowledge a merge lacks.

- **Early warnings.** Index every pushed branch and the functions it touches. Warn when two open branches change the same function, before either merges.
- **Semantic conflicts.** Branch A renames a function. Branch B adds a call to the old name. Git sees no conflict, both branches pass CI on their own, and the merged build breaks. With a call graph per branch, this is one query.
- **Shared resolution memory.** Git's `rerere` remembers resolutions on one machine only. Store each conflict and its resolution, keyed by content hash, and share them across the team and CI. If a resolution later proves wrong, record that, so it is no longer offered, while the history stays.
- **Reviewable resolutions.** Code written while resolving a conflict hides in the merge commit, and reviewers rarely look there. Record what each resolution changed beyond picking a side. Later, ask which bugs trace back to a conflict resolution.
- **Moves across files.** Syntax-aware merge drivers such as Mergiraf already handle many cases inside one file. The sidecar adds identity across files, so an edit can follow a function that moved.

Rebasing long stacks of commits is a different problem. Jujutsu solves it properly by treating conflicts as first-class objects. A sidecar can soften it with shared resolutions, and can work alongside Jujutsu.

### History index

Build the index once, one commit at a time, so queries never scan history. It answers questions git cannot:

- Every change to this function, across renames and moves between files.
- When did this function's signature last change, and which callers changed in the same commit? Which did not?
- Which files or functions always change together?
- Which commits added an `unsafe` block, a new dependency or a new public API?
- When did this error message first appear, and where is it used now?
- Whose changes still survive in this code?

MergeStat runs SQL over git history, Sourcegraph searches diffs, and CodeQL analyses one snapshot deeply. None of them combine entity identity, history and facts learned later.

### Knowledge layer

- **Facts learned later.** Bugs introduced, reverts and their reasons, advisories, review status. Valid time is the commit; transaction time is when the fact was learned.
- **Ref history.** A server hook records every branch move, so a force-push no longer erases shared history.

### Where the facts come from

Use three sources, and record which tool produced each fact:

- **Tree-sitter for all of history.** Cheap, works on code that no longer builds, good enough for structure and function boundaries.
- **SCIP indexers in CI for main-branch commits.** SCIP was built to index commits for code navigation. Indexers exist for Rust (`rust-analyzer scip`), Java, TypeScript, Python, C, C++ and more. They give compiler-quality references, call graphs and stable symbol names.
- **The developer's language server for their own branch.** It is already running. An editor plugin or a git hook sends facts for the branch on each commit or push. This is what makes early conflict warnings possible.

A live language server is a poor fit for history: it works on one checkout, needs time to start for each commit, and needs the project to build, which old commits often no longer do.

Re-indexing an old commit with a better tool adds facts with the same valid time and a later transaction time. Queries read the best answer, and the earlier answer stays queryable with `:as-of`. Upgrading an indexer becomes a correction, not a silent rewrite.

### Example

A function moves to another file. Each fact's valid time starts at the commit that made it true.

```datalog
(transact {:valid-from "2026-01-10"} [[:fn-parse-header :fn/name "parse_header"]])
(transact {:valid-from "2026-01-10" :valid-to "2026-03-02"} [[:fn-parse-header :fn/file "src/http.rs"]])
(transact {:valid-from "2026-03-02"} [[:fn-parse-header :fn/file "src/http/header.rs"]])

;; Where has this function lived, and since when?
(query [:find ?file ?from
        :valid-at :any-valid-time
        :where [:fn-parse-header :fn/file ?file]
               [:fn-parse-header :db/valid-from ?from]])
;; => "src/http.rs"        1768003200000   (2026-01-10)
;;    "src/http/header.rs" 1772409600000   (2026-03-02)
```

Two open branches, indexed against main. One renames the function, the other adds a call to it:

```datalog
(transact [[:chg-1 :change/branch "feature/rename-parser"]
           [:chg-1 :change/renames :fn-parse-header]
           [:chg-2 :change/branch "feature/retry"]
           [:chg-2 :change/adds-call-to :fn-parse-header]])

;; Semantic conflicts between open branches
(query [:find ?renamed-on ?called-on ?fn
        :where [?a :change/renames ?f]
               [?b :change/adds-call-to ?f]
               [?a :change/branch ?renamed-on]
               [?b :change/branch ?called-on]
               [?f :fn/name ?fn]])
;; => "feature/rename-parser" "feature/retry" "parse_header"
```

A bisect finds, weeks later, that a commit introduced a bug. The fact is about the commit, so its valid time is the commit's:

```datalog
(transact {:valid-from "2026-03-02"} [[:c-4f1a :commit/sha "4f1a9c2"]
                                      [:c-4f1a :commit/touches :fn-parse-header]])
(transact {:valid-from "2026-03-02"} [[:c-4f1a :commit/introduced :bug-812]
                                      [:bug-812 :bug/title "Header values cut at 1 KB"]])

;; What did we know before the bisect (transaction 5)?
(query [:find ?sha ?bug
        :as-of 5
        :where [?c :commit/introduced ?b]
               [?c :commit/sha ?sha]
               [?b :bug/title ?bug]])
;; => no results

;; What do we know now?
(query [:find ?sha ?bug
        :where [?c :commit/introduced ?b]
               [?c :commit/sha ?sha]
               [?b :bug/title ?bug]])
;; => "4f1a9c2" "Header values cut at 1 KB"
```

### Design issues

- **Scale.** A snapshot per commit explodes. Store only changes: each version of a function is valid from one commit until the next change. A large monorepo can still reach billions of facts.
- **History is a graph; valid time is a line.** Branches and merges do not map to one timeline. A practical start is the main branch's first-parent history as the valid-time axis, with other branches as overlays, as in the example above. This needs real design work.
- **Symbol identity.** Symbol names from SCIP or a language server stay stable only while the name does. Renames and moves need matching, for example by comparing function bodies. Store each match with a confidence score, so a wrong match can be corrected later.
- **Never replay a resolution blindly.** The code around it may have changed. Offer it, run the tests, and never apply it silently.
- **Server-side work.** Watching every pushed branch means running next to the git host, not only on laptops.
- **Not everything needs two time axes.** Early warnings and cross-file merges mostly need the graph. Shared resolutions, facts learned later and ref history are where the two axes earn their place.

## Supply chain history

Store each release's full dependency tree, and add vulnerability and license findings as they are published. The two time axes map cleanly:

- **Valid time** is when a release contained a package version.
- **Transaction time** is when you learned something about that package.

The advisory almost always arrives after the release. That gap is the point.

**Questions it answers**

- Which releases shipped with this package version, directly or through another package?
- When the release shipped, did we already know the version was vulnerable?
- How long did each affected release stay in use after the advisory was known?

**Why Minigraf fits.** Transitive dependencies are a recursive rule. "What we knew at ship time" is an `:as-of` query; "what was actually in the release" is a `:valid-at` query. Most tools keep only one of these. Inputs are easy to find: SBOM files (CycloneDX, SPDX), lockfiles and OSV advisories. Releases hang off the commits in the [git sidecar](#git-sidecar).

**Schema sketch.** Releases and packages share one `:depends-on` attribute, so one rule follows the whole tree. Advisories carry `:advisory/affects`.

```datalog
;; Transactions 1 and 2: release 1.4.0 ships on 2026-02-01.
(transact {:valid-from "2026-02-01"} [[:app-1-4 :release/tag "1.4.0"]
                                      [:app-1-4 :depends-on :http-2-3]])
(transact {:valid-from "2026-02-01"} [[:http-2-3 :package/name "http 2.3"]
                                      [:http-2-3 :depends-on :log-2-14]
                                      [:log-2-14 :package/name "log 2.14"]])

;; Transaction 3: months later, an advisory is published. The flaw
;; existed since the package's own release, so that is its valid time.
(transact {:valid-from "2025-11-01"} [[:adv-77 :advisory/id "ADV-2026-077"]
                                      [:adv-77 :advisory/affects :log-2-14]])

(rule [(dep ?a ?b) [?a :depends-on ?b]])
(rule [(dep ?a ?b) [?a :depends-on ?m] (dep ?m ?b)])

;; Did we know at ship time?
(query [:find ?tag ?pkg ?adv
        :as-of 2
        :valid-at "2026-02-01"
        :where [?r :release/tag ?tag]
               (dep ?r ?p)
               [?a :advisory/affects ?p]
               [?a :advisory/id ?adv]
               [?p :package/name ?pkg]])
;; => no results

;; Was the release affected, as we know now?
(query [:find ?tag ?pkg ?adv
        :valid-at "2026-02-01"
        :where [?r :release/tag ?tag]
               (dep ?r ?p)
               [?a :advisory/affects ?p]
               [?a :advisory/id ?adv]
               [?p :package/name ?pkg]])
;; => "1.4.0" "log 2.14" "ADV-2026-077"
```

## Test impact and flaky tests

Store which tests cover which code, and every test result per commit. Large CI systems build this in-house. A small team could keep it in one file.

**Questions it answers**

- Which tests must run for this change? This is reverse reachability from the changed functions.
- Is this test flaky, or did it start failing at a real commit?
- Which code had no covering test in a given release?

**Why Minigraf fits.** Test selection is a graph traversal from changed code to the tests that reach it, through call edges. Telling flaky from broken needs result history across commits. Coverage and calls come from the [git sidecar](#git-sidecar) index.

**Schema sketch.** `:covers` links a test to a function; `:calls` links functions. Each run is an entity with `:run/test`, `:run/commit` and `:run/outcome`.

```datalog
(transact [[:fn-auth :fn/name "check_token"]
           [:fn-cart :fn/name "add_to_cart"]
           [:fn-cart :calls :fn-auth]
           [:t-login :test/name "test_login"]
           [:t-login :covers :fn-auth]
           [:t-cart :test/name "test_cart"]
           [:t-cart :covers :fn-cart]
           [:fn-fmt :fn/name "format_price"]
           [:t-fmt :test/name "test_format"]
           [:t-fmt :covers :fn-fmt]])

(rule [(calls ?a ?b) [?a :calls ?b]])
(rule [(calls ?a ?b) [?a :calls ?m] (calls ?m ?b)])
(rule [(reaches ?t ?f) [?t :covers ?f]])
(rule [(reaches ?t ?f) [?t :covers ?m] (calls ?m ?f)])

;; check_token changed. Which tests must run?
(query [:find ?test
        :where [?f :fn/name "check_token"]
               (reaches ?t ?f)
               [?t :test/name ?test]])
;; => "test_login", "test_cart"

(transact [[:run-1 :run/test :t-cart]
           [:run-1 :run/commit "4f1a9c2"]
           [:run-1 :run/outcome "pass"]
           [:run-2 :run/test :t-cart]
           [:run-2 :run/commit "4f1a9c2"]
           [:run-2 :run/outcome "fail"]
           [:run-3 :run/test :t-login]
           [:run-3 :run/commit "4f1a9c2"]
           [:run-3 :run/outcome "fail"]])

;; Tests that both passed and failed on the same commit
(query [:find ?test ?commit
        :where [?a :run/test ?t]
               [?a :run/commit ?commit]
               [?a :run/outcome "pass"]
               [?b :run/test ?t]
               [?b :run/commit ?commit]
               [?b :run/outcome "fail"]
               [?t :test/name ?test]])
;; => "test_cart" "4f1a9c2"
```

`test_login` failed but never passed on that commit, so it is broken, not flaky.

## AI code provenance

Record which code an AI agent wrote, under which task and model, and every human edit after. Nothing is overwritten, so the full story of each function stays queryable.

**Questions it answers**

- Which code in this release is still unedited agent output?
- Which bugs trace back to code written for a given task or by a given model?
- Who last changed this function, and what did it look like before?

**Why Minigraf fits.** Provenance is a history question, and a bi-temporal store never loses history. Links between code, tasks, commits and bug reports form a graph. Code units come from the [git sidecar](#git-sidecar); it pairs naturally with agent memory.

**Schema sketch.** A code unit carries `:unit/name`, `:unit/author`, `:unit/model` and `:unit/task`. Each edit is an entity with `:edit/unit` and `:edit/author`. Bugs carry `:bug/traced-to`.

```datalog
(transact [[:u-1 :unit/name "billing::apply_discount"]
           [:u-1 :unit/author "agent"]
           [:u-1 :unit/model "model-a"]
           [:u-1 :unit/task "TASK-118"]
           [:u-2 :unit/name "billing::round_total"]
           [:u-2 :unit/author "agent"]
           [:u-2 :unit/model "model-a"]
           [:u-2 :unit/task "TASK-118"]
           [:u-3 :unit/name "billing::tax_rate"]
           [:u-3 :unit/author "dana"]])
(transact [[:edit-1 :edit/unit :u-2]
           [:edit-1 :edit/author "dana"]])
(transact [[:bug-9 :bug/title "Discount applied twice"]
           [:bug-9 :bug/traced-to :u-1]])

;; Agent-written code that no person has edited
(query [:find ?name
        :where [?u :unit/author "agent"]
               [?u :unit/name ?name]
               (not-join [?u] [?e :edit/unit ?u])])
;; => "billing::apply_discount"

;; Trace a bug back to the task and model that wrote the code
(query [:find ?bug ?task ?model
        :where [?b :bug/traced-to ?u]
               [?b :bug/title ?bug]
               [?u :unit/task ?task]
               [?u :unit/model ?model]])
;; => "Discount applied twice" "TASK-118" "model-a"
```

## Migration tracker

Track a long refactor, such as moving every call site off an old API. A scanner finds each call site on every commit and records it with its file and owner. The store holds the whole history of the migration.

**Questions it answers**

- How many call sites remain per team, and how has that changed over time?
- Did anyone add new uses of the old API after the migration started?
- Which sites remain right now?

**Why Minigraf fits.** Progress and regressions are history queries. Call sites link to files and owners, so grouping is a plain join. It is small and concrete, which makes it a good first project with Minigraf. Call sites can come from the [git sidecar](#git-sidecar) index.

**Schema sketch.** A call site carries `:site/file`, `:site/owner` and `:site/api`. Its valid time is the time the call existed in the code: a migrated site gets a `:valid-to`.

```datalog
;; The migration starts on 2026-05-01. Site s-1 is migrated on 2026-06-10.
(transact {:valid-from "2026-05-01"}
          [[:s-1 :site/file "src/pay/charge.rs"]
           [:s-1 :site/owner "payments"]
           [:s-1 :site/api "old_client" {:valid-from "2026-05-01" :valid-to "2026-06-10"}]
           [:s-2 :site/file "src/search/index.rs"]
           [:s-2 :site/owner "search"]
           [:s-2 :site/api "old_client"]
           [:s-3 :site/file "src/pay/refund.rs"]
           [:s-3 :site/owner "payments"]
           [:s-3 :site/api "old_client"]])

;; On 2026-07-01, someone adds a new use.
(transact {:valid-from "2026-07-01"}
          [[:s-4 :site/file "src/pay/payout.rs"]
           [:s-4 :site/owner "payments"]
           [:s-4 :site/api "old_client"]])

;; Remaining sites per team on 2026-06-15
(query [:find ?owner (count ?s)
        :valid-at "2026-06-15"
        :where [?s :site/api "old_client"]
               [?s :site/owner ?owner]])
;; => "payments" 1, "search" 1

;; Uses added after the migration started
(query [:find ?file ?from
        :valid-at :any-valid-time
        :where [?s :site/file ?file]
               [?s :site/api "old_client"]
               [?s :db/valid-from ?from]
               [(> ?from 1777593600000)]])   ;; 2026-05-01 in Unix ms
;; => "src/pay/payout.rs" 1782864000000
```

## Verification results

Formal verifiers such as Verus, Creusot, Kani and Dafny prove properties of code for every input. Most of them keep nothing between runs, or keep only a cache keyed by a hash of each proof obligation. A key-value file does that job well. What gets thrown away is the history:

- **Results across versions.** Which functions were proved, failed or unresolved at each commit, and when that changed.
- **Dependencies between proofs.** Modular verification proves a caller using its callees' contracts, not their bodies. Some proofs also rest on contracts that are assumed, not proved, such as a contract on a system call. Together these form a graph.
- **Counterexamples.** The inputs that broke a proof, and the commit that fixed it.
- **Solver behaviour.** Proof time and timeouts per obligation over time.

**Questions it answers**

- This contract changed. Which proofs must run again, directly or through a chain?
- When the release shipped, which proved functions rested on a contract nobody proved?
- Which obligations got slower over the last 50 commits? Rising proof times warn of a proof that will soon time out.

**Why Minigraf fits.** Valid time is the commit a result is about. Transaction time is when the verifier ran. Reverse dependencies are a recursive rule. One file per repository is easy to build in CI and keep with the build artifacts.

**Schema sketch.** A proof entity per function and commit: `:proof/function`, `:proof/status`, `:proof/uses` (another proof whose contract it relies on), `:proof/trusts` (an assumed contract), plus duration and counterexample values. Contracts carry `:contract/function`.

```datalog
(transact {:valid-from "2026-06-01"}
          [[:p-inc :proof/function "inc"]
           [:p-inc :proof/status "verified"]
           [:p-inc2 :proof/function "inc_twice"]
           [:p-inc2 :proof/status "verified"]
           [:p-inc2 :proof/uses :p-inc]
           [:p-add3 :proof/function "add_three"]
           [:p-add3 :proof/status "verified"]
           [:p-add3 :proof/uses :p-inc2]
           [:p-avg :proof/function "average_reading"]
           [:p-avg :proof/status "verified"]
           [:p-avg :proof/trusts :k-sensor]
           [:k-sensor :contract/function "read_sensor"]])

(rule [(relies ?p ?q) [?p :proof/uses ?q]])
(rule [(relies ?p ?q) [?p :proof/uses ?m] (relies ?m ?q)])

;; The contract of inc changed. Which proofs must run again?
(query [:find ?caller
        :where [?q :proof/function "inc"]
               (relies ?p ?q)
               [?p :proof/function ?caller]])
;; => "inc_twice", "add_three"

;; On the release date, which proofs rested on an assumed contract?
(query [:find ?proved ?assumed
        :valid-at "2026-06-15"
        :where [?p :proof/trusts ?k]
               [?p :proof/status "verified"]
               [?p :proof/function ?proved]
               [?k :contract/function ?assumed]])
;; => "average_reading" "read_sensor"
```

**How to feed it.** No verifier needs to change. An adapter reads the verifier's machine-readable output after each CI run and writes facts. Keep large blobs, such as the obligation formulas, out of the graph: store them as files and reference them by hash.

**Limits.** The solver never touches the database. Minigraf stores results; it does not help prove anything. A plain proof cache is better served by a key-value file, and per-run timing at high volume is time-series data. Few teams run formal verifiers today, but the same adapter shape works for any CI result. AI agents now write proofs too, and retry many times. A record of failed attempts and their counterexamples is agent memory with an audit trail.
