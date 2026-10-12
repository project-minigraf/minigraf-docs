# Site search (#8): implementation plan

Spec: [`docs/specs/2026-10-12-search-design.md`](../specs/2026-10-12-search-design.md).
Branch `search/8-impl`, worktree `.worktrees/search-impl`, one PR to `main`.

Each task is test-first: write the test, see it fail, implement, see it pass. After every task:
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` (what the Pages
workflow runs). Tasks that touch `assets/` or `e2e/` also run the site build and `npm test`.

## Deviations from the spec

1. **Tokenizer module.** The spec has the Node test import `assets/search.js`. That module needs a
   DOM (it imports `docs.js`, which reads `document` on load), so the tokenizer and stopword list go
   in a pure module, `assets/tokenize.js`, which `search.js` imports and the Node test imports
   directly. The Rust stopword-parity test reads `assets/tokenize.js`.
2. **Search field without JavaScript.** The spec has no fallback. The field is rendered with the
   `hidden` attribute and `search.js` shows it, so a page without JavaScript has no dead control.

## Task 1: remove the benchmark

- Delete `src/bin/search_bench.rs` and the `default-run` line in `Cargo.toml` (one binary again).
- Check: `cargo run -- build` still works.

## Task 2: tokenizer (Rust)

- `tests/tokenize.json`: `[{"input": "...", "tokens": ["..."]}, …]`, at least the cases the spec
  lists: case folding; `valid-at` → `valid`, `valid-at` (`at` is a stopword); `as-of` → `as-of`;
  `stg-028` → `stg`, `028`, `stg-028`; `a-b` → `a-b`; `--x--` → nothing; `x-y-z` → `x-y-z`;
  stopwords and `and`/`not`/`or` kept; two-character minimum (`x`, `id`); `:frag/blob` →
  `frag`, `blob`; `[?x :a/b]` → nothing; numbers; repeats kept in order; non-ASCII (`café` →
  `caf`; `Größe` → `gr`, because `ö` and `ß` split the word and `e` is too short).
- Token order within a word: its parts in order, then the joined compound (`valid-at` → `valid`,
  `valid-at`). The spec leaves this open; the vectors fix it.
- `src/tokenize.rs`: `pub const STOPWORDS: &[&str]` (the spec's list), `pub fn tokenize(&str) ->
  Vec<String>` per the spec's four steps.
- Tests in `tokenize.rs`: every vector in `tests/tokenize.json` (read with `include_str!`, parsed
  with `serde_json`, added as a dev-dependency); the stopword list equals the `STOPWORDS` array in
  `assets/tokenize.js` (read with `include_str!`, extracted between `STOPWORDS = [` and `]`).
  Task 3 creates the JS file; until then that test fails, so tasks 2 and 3 land together.

## Task 3: tokenizer (JavaScript) and Node test

- `assets/tokenize.js`: `export const STOPWORDS = [...]` (one string per entry, same order),
  `export function tokenize(s)`.
- `e2e/tokenize.test.mjs`: reads `../tests/tokenize.json`, asserts `tokenize(input)` deep-equals
  `tokens` for each case.
- `e2e/package.json`: `"test": "node --test tokenize.test.mjs site.test.mjs"`.

## Task 4: text extraction in `render()`

- `Rendered` gains `text: String` (body) and `heading_text: String`, filled from the events per the
  spec: `Text` and `Code` events and code-block text, each followed by a space; heading `Text` and
  `Code` go to both fields; `Html`/`InlineHtml` skipped. Link text is `Text` inside a link and is
  included.
- Tests in `render.rs`: a fragment with a heading, a paragraph with inline code and a link, a
  Datalog code block and an inline `<span>` gives the expected `text` and `heading_text`; raw HTML
  tags and attributes do not appear.

## Task 5: the index (`src/search.rs`)

- `pub struct SearchIndex { terms: BTreeMap<String, BTreeMap<String, i64>>, anchors:
  BTreeMap<String, String> }`.
- `SearchIndex::build(blobs: &HashMap<String, Rendered>)`: weights per the spec (1 per body
  occurrence, +10 per heading occurrence); anchor = `ids[0]` if `heading.is_some()` else `""`.
- `fn chunks(&self) -> Vec<(String, String)>`: (term, list) pairs, lists `"<blob>:<weight>"`
  joined by spaces in blob-hash order, split so no list exceeds `CHUNK_BYTES = 3000`.
- `fn build_id(&self) -> String`: SHA-256 of the canonical text (spec: `<term>\t<full list>\n` per
  term, then `<hash>\t<anchor>\n` per blob), first 16 hex digits.
- `fn write(&self, path, build_id)`: creates the file (removing any old file and WAL, as
  `DocsGraph::create` does), transacts chunks (one entity per chunk, entity idents
  `:search/c<n>`), blobs (`:search/b<n>` with `:blob/hash`, `:blob/anchor`) and the build entity
  (`:search/build` with `:build/id`), all with the window `[v0.0.0, forever)`, in transactions of
  at most 2,000 facts, then checkpoints.
- `pub fn verify(path, expected_id, frag_blobs: &BTreeSet<String>) -> Result<()>`: reopens the
  file and fails unless the three checks in the spec hold.
- Tests: weights and anchors on a two-blob corpus; a term in more blobs than fit in 3,000 bytes
  spans chunks and every (term, blob) appears once; a 3,000-byte list writes to a file-backed
  database (tempdir under `target/`, no new dependency); `build_id` is stable across two builds and
  changes when one blob's text changes; write then verify passes; verify fails with a wrong id, with
  a blob missing from `frag_blobs`, and with an extra blob in `frag_blobs`.

## Task 6: build id in `docs.graph`

- `DocsGraph::write_build_id(&str)` (entity `:docs/build`, `:build/id`, window ALL) and
  `DocsGraph::build_id()` / `DocsGraph::all_frag_blobs()` (both `:any-valid-time`).
- Extend the `graph.rs` test: round trip of the id; `all_frag_blobs` returns blobs from every
  window.

## Task 7: wire the build (`main.rs`)

- After `load_sources`: `let index = SearchIndex::build(&sources.blobs); let id = index.build_id();`
- Write `docs.graph` as today plus `write_build_id(&id)` before its checkpoint.
- Write `search-<id>.graph`, then `search::verify(path, &id, &graph.all_frag_blobs()?)`, and check
  that `graph.build_id()` equals `id`.
- Factor the gzip step into `fn gzip(path)` and use it for both graphs.
- Log line adds `search-<id>.graph <n> KiB`.
- Check: the site builds; `dist/` holds exactly one `search-*.graph` and its `.gz`.

## Task 8: markup (`site.rs`)

- In `topbar`, before the "What changed" link: a `<div class="search" hidden>` holding a toggle
  button (`aria-label="Search"`, `aria-expanded="false"`), the input (`type="search"`,
  `role="combobox"`, `aria-expanded="false"`, `aria-controls="search-results"`,
  `aria-autocomplete="list"`, `aria-label="Search the docs for <tag>"`, `autocomplete="off"`), the
  `<ul id="search-results" role="listbox" hidden>` and a visually hidden
  `<div class="sr-only" role="status" id="search-status">`.
- Load `assets/search.js` as a module on doc, diff and console pages (added to each page's `extra`
  head string).
- Rust test (new, in `site.rs`): the doc page's HTML contains the combobox attributes and the
  script tag.

## Task 9: `assets/search.js`

Per the spec's [Querying](../specs/2026-10-12-search-design.md#querying) and UI sections:

- `loadIndex()`: once; awaits `getDb()`, reads the build id with
  `[:find ?id :any-valid-time :where [?b :build/id ?id]]`, fetches `search-<id>.graph.gz` (via
  `DecompressionStream`, falling back to `search-<id>.graph`), imports into a second
  `BrowserDb.openInMemory()`, checks its `:build/id`, loads the anchor map. Any failure → the
  update message in the status region and the list. `docs.js` exports the gzip fetch (today's
  `fetchGraph` generalized to a name) for reuse.
- `search(q, tag)`: tokenize, dedupe, exact/prefix queries, AND, scores, the per-version join
  (cached per tag), result order, first 20.
- Rendering: options with `id`, `data-href`, title (`inlineTitle`), heading, snippet. Snippets
  load after the list renders: `fetchBlob`, `textContent` via a detached element, ~160 characters
  around the first match, each token's matches wrapped in `<mark>` after escaping.
- Input handling: debounce 120 ms; ignore stale responses (sequence number).
- Keyboard and ARIA per the spec; `/` shortcut ignored when focus is in an input, textarea, select
  or contenteditable.
- Version: `document.body.dataset.version` at each search; the field's `aria-label` is updated on
  focus and on each search.
- Shows the `.search` container on load.

## Task 10: styles (`style.css`)

- Field in the top bar between the version picker and the links; results panel under it, up to
  70vh with scrolling, both themes via the existing tokens; selected option highlighted; `<mark>`
  styled for both themes; `.sr-only` utility if missing.
- At ≤ 900px the field collapses to the toggle button; when expanded it covers the top bar width.
- Check the "pages fit a phone screen" e2e test still passes.

## Task 11: end-to-end tests (`e2e/site.test.mjs`)

The spec's Playwright cases: a common term returns results; `LogWriter` on v3.0.0 has results and
on v2.0.4 none; prefix (`recurs` finds `recursive`); AND of two terms; first result's
`data-href` is `<tag>/<slug>/#<anchor>` and Enter navigates there; `/`, arrows, Escape (once
closes, twice clears); ARIA attributes (`aria-expanded`, `aria-activedescendant`,
`aria-selected`); in-place switch to v2.0.4 then search for `LogWriter` gives none; with
`page.route` answering 404 for `search-*.graph*`, the update message appears.

## Task 12: documentation

- `DESIGN.md`: "Search" section (index layout, tokenizer, build id and pairing, browser flow) and
  the two files in the output table.
- `README.md`: one line on search if it lists site features.
- Update the spec's status line to "implemented" and note the two deviations above.

## Finish

- Full local run: fmt, clippy, `cargo test`, site build, `npm test`.
- Codex review of the PR (`codex exec -s read-only` from the worktree); verify findings, fix.
- Open the PR (`Refs #8`, closes on merge: `Closes #8` — the docs repo has no release-gated
  issues), monitor CI until green, then ask before merging.
