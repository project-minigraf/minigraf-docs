# Site search (#8)

Status: draft for review, 2026-10-12.

## Goal

Readers can search the docs for keywords. Results come from the version they are reading:
searching v2.0.4 never returns text that exists only in v3.0.0, and the other way round.

Out of scope: search without JavaScript (crawlers do not search), server-side search, third-party
search boxes, stemming, fuzzy matching, non-ASCII tokenization (see [Limits](#limits)).

## Decisions so far

| Decision | Choice | Why |
|---|---|---|
| Where the index lives | A Minigraf file, queried with Datalog in the browser | Same engine and format as `docs.graph`; no new dependency |
| Layout | One entity per term (chunked), blob list in a string ("layout B") | Benchmark below |
| File | Separate `search.graph`, loaded on first use of the search box | Keeps the page-load download at today's size |
| No-JavaScript fallback | None | Crawlers do not search |

### Benchmark

Corpus at commit 27a50b7: 789 blobs, 5,927 terms, 47,776 (term, blob) postings. minigraf 2.0.4
(format v7). Browser times are Chromium with the vendored `@minigraf/browser`. The prototype is
`src/bin/search_bench.rs` on branch `search/8-index-bench`; it is removed before merge.

| Layout | File | gzip | Import | One term | Two terms |
|---|---|---|---|---|---|
| none (today's `docs.graph`) | 1.3 MB | 342 KB | 4 ms | | |
| A: one entity per posting | 46.7 MB | 11.9 MB | 243 ms | 320–450 ms | 670–780 ms |
| B: one entity per term chunk | 8.2 MB | 2.0 MB | 9 ms | 23–29 ms | 45–51 ms |
| B2: B with numbered blobs | 6.4 MB | 1.8 MB | 7 ms | 22–31 ms | 55–57 ms |

B2 saves 11% of the download for an extra lookup and slower two-term queries, so B is chosen.
B's sizes were measured with the index inside a copy of `docs.graph`; a separate `search.graph`
holds only the index, so it is smaller by about the size of `docs.graph`.

## What is indexed

Every blob in `blobs/` (authored and snapshot pages alike). A blob is version-free: which versions
contain it is already recorded by the `:frag/blob` facts in `docs.graph`. The search index is
therefore version-free too, and version scoping comes from joining with `docs.graph` at
`:valid-at` (see [Querying](#querying)).

### Tokens

Applied to a blob's visible text (HTML tags removed, the five entities `&lt; &gt; &quot; &#39;
&amp;` decoded) and to a query alike:

1. Lower-case (ASCII only; other characters are left as they are and act as separators below).
2. Split on every character that is not ASCII alphanumeric and not `-`.
3. Each piece is split on `-`. Every part of 2 or more characters that is not a stopword is a
   token. If a piece has two or more non-empty parts, the parts joined with `-` are also a token
   (`valid-at`, `not-join`, `stg-028`), whatever their length.
4. Stopwords: a fixed list of about 50 English function words (articles, prepositions,
   pronouns, auxiliaries). `and`, `not` and `or` are **not** stopwords, because they are Datalog
   clause names readers search for.

The exact stopword list is fixed in the implementation plan and covered by the shared test
vectors (see [Keeping things in sync](#keeping-things-in-sync)).

### Weights

For each (term, blob): 1 per occurrence in the text, plus 10 per occurrence inside an `h2`–`h4`
heading. Weights are only used for ordering results.

## `search.graph`

A Minigraf file in the same format as `docs.graph` (v7 while the builder pins minigraf 2.0.x),
written by the builder after `docs.graph`. All facts have the window `[v0.0.0, forever)` and are
queried with `:any-valid-time`.

| Entity | Attributes |
|---|---|
| term chunk | `:term/text` (the token), `:term/blobs` (string: `"<blob>:<weight> <blob>:<weight> …"`) |
| blob | `:blob/hash`, `:blob/anchor` (id of the blob's first heading, or `""`) |
| build | `:build/id` (string, see below) |

- A term whose list is longer than 3,000 bytes is split over several chunk entities with the same
  `:term/text`. 3,000 stays under v7's per-fact limit (`MAX_FACT_BYTES`, 4,080 bytes) with room for
  the other fields. Each chunk is its own entity, so no entity holds two values of one attribute
  from one transaction (v2.x known issue #371).
- `:blob/anchor` lets a result link to `<tag>/<slug>/#<anchor>`. Blobs without a heading (a page's
  first fragment) link to the page.
- `search.graph.gz` is written next to it, as for `docs.graph`.

## Keeping things in sync

Two things can drift: the two graph files, and the tokenizer in the builder (Rust) and in the
browser (JavaScript). Each gets a check.

### Graphs: build id

The builder computes `build id = first 16 hex digits of SHA-256(sorted blob hashes, newline-separated)`
and writes it as `:build/id` into **both** `docs.graph` and `search.graph` (in `docs.graph` as a
new entity, window `[v0.0.0, forever)`).

- **Build time:** after writing both files the builder reopens `search.graph` and fails the build
  unless (a) its `:build/id` equals `docs.graph`'s, (b) its set of `:blob/hash` values equals the
  set of blobs referenced by `:frag/blob` in `docs.graph`, and (c) every blob hash named in a
  `:term/blobs` list is one of those.
- **Browser:** after loading `search.graph`, the page compares the two build ids. On a mismatch
  (a deploy happened between the two fetches) it refetches both files once, bypassing the cache
  (`cache: "reload"`), and replaces both in-memory databases. If they still differ, the search box
  shows "The search index is being updated. Reload the page in a minute." and returns no results.
- **E2E:** a Playwright test serves a `search.graph` with a different build id and checks for the
  message.

### Tokenizer: shared test vectors

`tests/tokenize.json` holds input strings and their expected tokens (cases: case folding,
compounds, stopwords, two-character minimum, numbers, entities, punctuation such as `:frag/blob`
and `[?x :a/b]`, non-ASCII). Both a Rust unit test (`tokenize.rs`) and a Node test
(`e2e/tokenize.test.mjs`, importing `assets/search.js`) must produce exactly those tokens. The Pages
workflow already runs `cargo test` and `npm test` on every pull request; `npm test` is extended to
run `tokenize.test.mjs` as well as `site.test.mjs`. A change to one tokenizer without the other
fails CI.

## Querying

In the browser (`assets/search.js`):

1. On first focus of the search box (or the `/` shortcut), fetch `search.graph.gz` (falling back
   to `search.graph`) and import it into a second `BrowserDb.openInMemory()` handle. Check the
   build id. `docs.graph` is loaded as today.
2. Tokenize the query. Every token is matched exactly except the last, which is matched as a
   prefix (search as you type), unless the query ends in a space. A query with no tokens shows
   nothing.
3. For each token, query `search.graph`:
   - exact: `[:find ?blobs :any-valid-time :where [?t :term/text "<token>"] [?t :term/blobs ?blobs]]`
   - prefix: `[:find ?x ?blobs :any-valid-time :where [?t :term/text ?x] [(starts-with? ?x "<token>")] [?t :term/blobs ?blobs]]`

   Parse each list into blob → weight, summing over chunks and, for prefixes, over matching terms.
4. Keep blobs present for every token (AND). Score = sum of the token weights.
5. Join with `docs.graph` at the version being read. One query per version, cached:
   `[:find ?slug ?order ?blob ?heading :valid-at "<version time>" :where [?f :frag/blob ?blob] [?f :frag/page ?slug] [?f :frag/order ?order] [?f :frag/heading ?heading]]`
   plus the nav query that already exists for titles. A blob in several pages yields one result per
   page; a blob not in the version is dropped.
6. Order by score (descending), then page order, then fragment order. Show the first 20.

The version is the one the page shows (`body.dataset.version`, which changes on an in-place
version switch). On the diff and console pages, it is the latest release.

## UI

- A search field in the top bar, before "What changed". On narrow screens it collapses to an icon
  that expands the field.
- Results drop down under the field: page title, the fragment's heading (`:frag/heading`) if any,
  and a snippet. The snippet comes from the blob's HTML (fetched through the existing `fetchBlob`
  cache, for the visible results only): the text around the first match, about 160 characters,
  with matches marked by `<mark>`.
- Keyboard: `/` focuses the field; arrow keys move through results; Enter opens the selected (or
  first) result; Escape closes the list.
- While `search.graph` loads, the list shows "Loading the search index…".
- Selecting a result is ordinary navigation to `<tag>/<slug>/#<anchor>`.

## Build output

`dist/search.graph`, `dist/search.graph.gz`. The build log line reports the size of both graphs.
`DESIGN.md` gets a "Search" section (index layout, build id, tokenizer) and the output table gains
the two files.

## Tests

- Rust: tokenizer vectors; chunking (a term over 3,000 bytes spans chunks, every blob appears once);
  index round trip on a small corpus (query a term, get the expected blobs and weights); the build
  checks fail on a mismatched build id, on a missing blob, on an unknown blob.
- Node: tokenizer vectors.
- Playwright (against the built site): a term present in all versions returns results; a v3-only
  term (for example `LogWriter`) returns results on v3.0.0 and none on v2.0.4; prefix matching; two
  terms are ANDed; keyboard navigation and Enter; switching version in place then searching scopes
  to the new version; the build-id mismatch message.

## Limits

- Tokens are ASCII; non-ASCII letters split words. The docs are English and code.
- No stemming: `rule` does not match `rules` unless typed as a prefix.
- The index grows with the number of distinct blobs, not versions; a new release that changes
  little adds little.

## Follow-ups (not in this change)

- Querying `search.graph` from the Datalog console.
- When the builder moves to minigraf 3 (format v8: dictionary-encoded ids, long strings stored
  once), re-measure; layout A may become viable.
