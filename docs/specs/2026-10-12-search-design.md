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
| File | Separate `search-<build id>.graph`, loaded on first use of the search box | Keeps the page-load download at today's size |
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
B's sizes were measured with the index inside a copy of `docs.graph`; a separate search graph
holds only the index, so it is smaller by about the size of `docs.graph`.

## What is indexed

Every blob the build renders (authored and snapshot pages alike), taken from the builder's
in-memory blob map, not from files in `blobs/`. Every such blob is referenced by at least one
`:frag/blob` fact. A blob is version-free: which versions contain it is recorded by those facts
in `docs.graph`. The search index is therefore version-free too, and version scoping comes from
joining with `docs.graph` at `:valid-at` (see [Querying](#querying)).

### Text

Extracted at build time, in `render()`, from the same pulldown-cmark events that produce the
HTML, so no HTML parsing or entity decoding is needed:

- **Body text:** every `Text` and `Code` event, and the text of code blocks, each followed by a
  space. Raw HTML events (`Html`, `InlineHtml`) are skipped.
- **Heading text:** the `Text` and `Code` events inside any heading. Heading text is part of the
  body text as well.

`Rendered` gains `text` and `heading_text` fields. Snapshot pages go through `render()` too.

### Tokens

`tokenize(s)` is the one function that must behave the same in Rust (builder) and JavaScript
(browser, applied to queries):

1. Lower-case ASCII letters. Other characters are unchanged.
2. Split on every character that is not ASCII alphanumeric and not `-`. Call each piece a word.
3. Split each word on `-` and drop empty parts. Each part of 2 or more characters that is not a
   stopword is a token. If a word has two or more parts, the parts joined with `-` are also a
   token (`valid-at`, `as-of`, `not-join`, `stg-028`), even if the parts are stopwords or short.
4. Tokens are returned in order of appearance, with repeats (the builder counts them; the browser
   removes duplicates).

Stopwords (normative):
`a an are as at be but by can do does for from has have if in into is it its no of on one so than
that the their then there these this to was we were when which while who will with you your`.
`and`, `not` and `or` are not stopwords: they are Datalog clause names readers search for.

### Weights

For each (term, blob): the number of times the term occurs in the body text, plus 10 for each
time it occurs in the heading text. Weights are only used to order results.

## `search-<build id>.graph`

A Minigraf file in the same format as `docs.graph` (v7 while the builder pins minigraf 2.0.x),
written by the builder after `docs.graph`. Its name carries the build id (see
[Keeping things in sync](#keeping-things-in-sync)). All facts have the window
`[v0.0.0, forever)` and are queried with `:any-valid-time`.

| Entity | Attributes |
|---|---|
| term chunk | `:term/text` (the token), `:term/blobs` (string: `"<blob>:<weight> <blob>:<weight> …"`, blobs in hash order) |
| blob | `:blob/hash`, `:blob/anchor` (the id of the heading the blob starts with, or `""`) |
| build | `:build/id` |

- **Chunking.** A term's list is split so that no `:term/blobs` string exceeds 3,000 bytes; each
  chunk is its own entity with the same `:term/text`. The per-fact limit in format v7 is
  `MAX_FACT_BYTES` = 4,080 bytes (minigraf 2.0.4, `storage/packed_pages.rs`) and applies to the
  serialized fact. The other fields of a fact (entity id, attribute, transaction and valid-time
  fields, framing) take well under 1,000 bytes. A fact over the limit is rejected when the
  builder writes it, so an overflow fails the build instead of shipping; a Rust test writes a
  maximal chunk to a file-backed database. One entity per chunk also means no entity holds two
  values of one attribute from one transaction (v2.x known issue #371).
- **Anchor.** `:blob/anchor` is `Rendered.ids[0]` when the fragment starts with a heading
  (`Rendered.heading` is set), else `""`. A result links to `<tag>/<slug>/#<anchor>`, or to the
  page when the anchor is empty.
- A gzipped copy, `search-<build id>.graph.gz`, is written next to it, as for `docs.graph`.

## Keeping things in sync

Two things can drift: the two graph files, and the tokenizer in the builder (Rust) and in the
browser (JavaScript). Each gets a check.

### Graphs: build id in the file name

**Build id:** the first 16 hex digits of the SHA-256 of the index's canonical text: one line
`<term>\t<full blob list>` per term in term order, then one line `<hash>\t<anchor>` per blob in
hash order. It changes whenever the index content changes, including after a tokenizer change.
It does not depend on transaction timestamps, so an unchanged corpus gives the same id.

The builder writes `:build/id` into `search-<id>.graph` and into `docs.graph` (a new entity,
window `[v0.0.0, forever)`).

- **The pair is pinned.** The browser reads the build id from the `docs.graph` it already loaded
  and fetches `search-<id>.graph.gz` (falling back to `search-<id>.graph`). It never fetches a
  search file by a fixed name, so it cannot pair a `docs.graph` with a search index from another
  build. The file is immutable under its name, so the default HTTP cache is fine.
- **Deploy between page load and search.** GitHub Pages replaces the whole site on deploy, so the
  old `search-<id>.graph` is gone. The fetch then fails (404) and the search box shows "The docs
  were updated since this page loaded. Reload the page to search." `docs.graph` is revalidated on
  every page load (`cache: "no-cache"`, as today), so a reload picks up the new id.
- **Browser assertion.** After import, the page checks that the search graph's `:build/id` equals
  the requested id, and shows the same message if not.
- **Build time.** After writing both files the builder reopens the search graph and fails unless
  (a) its `:build/id` equals the one in `docs.graph` and in its file name, (b) its set of
  `:blob/hash` values equals the set of `:frag/blob` values in `docs.graph` (queried with
  `:any-valid-time`), and (c) every blob named in a `:term/blobs` list is in that set. `dist/`
  holds exactly one `search-*.graph` (the output directory is cleared at the start of a build, as
  today).
- **E2E.** A Playwright test removes the search file from a copy of the built site and checks the
  message.

### Tokenizer: shared test vectors

`tests/tokenize.json` holds input strings and their expected tokens. Cases: case folding,
compounds (including all-stopword and one-character parts), stopwords, the two-character minimum,
numbers, punctuation such as `:frag/blob` and `[?x :a/b]`, leading/trailing/doubled `-`, and
non-ASCII text. Both a Rust unit test (`src/tokenize.rs`) and a Node test
(`e2e/tokenize.test.mjs`, importing `assets/search.js`) must produce exactly those tokens. The
Pages workflow already runs `cargo test` and `npm test` on every pull request; `npm test` is
extended to run `tokenize.test.mjs` as well as `site.test.mjs`. A change to one tokenizer without
the other fails CI. A Rust test also checks that the stopword list in `tokenize.rs` and the one in
`assets/search.js` are equal (it reads the JS file).

## Querying

In the browser (`assets/search.js`). The `starts-with?` predicate and `:any-valid-time` used below
were exercised against the vendored `@minigraf/browser` in the benchmark.

1. On first focus of the search field (or the `/` shortcut), wait for `docs.graph` (loaded as
   today), read its build id, fetch the search graph and import it into a second
   `BrowserDb.openInMemory()` handle. Load the anchors once:
   `[:find ?h ?a :any-valid-time :where [?b :blob/hash ?h] [?b :blob/anchor ?a]]`.
2. Tokenize the query and remove duplicate tokens. Every token is matched exactly except the
   last, which is matched as a prefix (search as you type) unless the query's last character is
   not ASCII alphanumeric (for example a space). A query with no tokens shows nothing.
3. For each token, query the search graph:
   - exact: `[:find ?t ?blobs :any-valid-time :where [?t :term/text "<token>"] [?t :term/blobs ?blobs]]`
   - prefix: `[:find ?t ?blobs :any-valid-time :where [?t :term/text ?x] [(starts-with? ?x "<token>")] [?t :term/blobs ?blobs]]`

   (`?t` is in `:find` so two chunks with equal lists are not collapsed into one row.) Parse the
   lists into blob → weight, summing over chunks and, for a prefix, over the matching terms.
4. Keep the blobs found for every token (AND). A blob's score is the sum of its weights over the
   tokens.
5. Join with `docs.graph` at the version being read, once per version (cached):
   `[:find ?slug ?order ?blob ?heading :valid-at "<version time>" :where [?f :frag/blob ?blob] [?f :frag/page ?slug] [?f :frag/order ?order] [?f :frag/heading ?heading]]`,
   plus the existing nav query for page titles and page order.
6. A result is a (page, blob) pair. A blob that occurs twice on one page gives one result, at its
   lowest fragment order; a blob on two pages gives two results; a blob not in the version gives
   none. Order: score (descending), then page order, then slug, then fragment order. This order
   is total. Show the first 20.

The version is the one the page shows (`body.dataset.version`, which changes on an in-place
version switch). On the diff and console pages, it is the latest release.

## UI

- **Field.** A search field in the top bar, before "What changed", with `aria-label` "Search the
  docs for <version>" (updated on a version switch). On narrow screens it collapses to a button
  (`aria-label` "Search", `aria-expanded`) that reveals the field and focuses it.
- **Combobox.** The field follows the WAI-ARIA combobox pattern: `role="combobox"`,
  `aria-expanded`, `aria-controls` pointing at the results list, `aria-autocomplete="list"`, and
  `aria-activedescendant` for the selected result. The list has `role="listbox"`; each result is
  a `role="option"` containing a link, with `aria-selected` on the selected one.
- **Announcements.** A visually hidden `role="status"` region reads "Loading the search index…",
  "<n> results" or "No results", and the update message from
  [Keeping things in sync](#keeping-things-in-sync).
- **Results** show the page title, the fragment's heading (`:frag/heading`) if any, and a snippet:
  the text around the first match in the blob (from the existing `fetchBlob` cache, fetched for
  the shown results only), about 160 characters, with matches in `<mark>`.
- **Keyboard.** `/` focuses the field (unless focus is already in a text input); Up/Down move
  the selection; Enter opens the selected result, or the first if none is selected; Escape closes
  the list and keeps focus in the field; a second Escape clears the field. Tab leaves the field
  and closes the list.
- Opening a result is ordinary navigation.

## Build output

`dist/search-<id>.graph`, `dist/search-<id>.graph.gz`. The build log line reports the size of both
graphs. `DESIGN.md` gets a "Search" section (index layout, build id, tokenizer) and the output
table gains the two files.

## Tests

- **Rust:** tokenizer vectors; stopword lists equal in `tokenize.rs` and `assets/search.js`; text
  extraction (body and heading text, raw HTML skipped); chunking (a term over 3,000 bytes spans
  chunks, every (term, blob) pair appears once); a maximal chunk fact writes to a file-backed
  database; build id is deterministic across two builds of the same corpus and changes when one
  blob changes; index round trip on a small corpus (a term gives the expected blobs, weights and
  anchors); the build checks fail on a mismatched build id, a missing blob and an unknown blob.
- **Node:** tokenizer vectors.
- **Playwright** (against the built site): a term present in every version returns results; a
  v3-only term (`LogWriter`) returns results on v3.0.0 and none on v2.0.4; prefix matching; two
  terms are ANDed; a result links to the right page and anchor; keyboard (`/`, arrows, Enter,
  Escape); the combobox and listbox ARIA attributes; switching version in place then searching
  scopes to the new version; with the search file removed, the update message appears.

## Limits

- Tokens are ASCII; non-ASCII letters split words. The docs are English and code.
- No stemming: `rule` does not match `rules` unless typed as a prefix.
- The index grows with the number of distinct blobs, not versions; a new release that changes
  little adds little.

## Follow-ups (not in this change)

- Querying the search graph from the Datalog console.
- When the builder moves to minigraf 3 (format v8: dictionary-encoded ids, long strings stored
  once), re-measure; layout A may become viable.
