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
