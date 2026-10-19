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
