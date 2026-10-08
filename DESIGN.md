# minigraf-docs design

Versioned documentation for Minigraf, indexed by Minigraf itself.

## Goals

- One site for every release from v2.0.0 on, plus the unreleased v3.0.0.
- Content shared between versions is written and stored once.
- Pages render on the server (at build time) so they work without JavaScript and are indexed.
- After load, the browser uses `@minigraf/browser` to switch versions in place, show what
  changed between two versions, and run Datalog against the docs index.
- Static output only. Hosted on GitHub Pages, but `dist/` works on any static host.

## Version time

Each release maps to a point on Minigraf's valid-time axis. Release dates cannot be used:
v2.x patches ship after v3.0.0 work starts, so a later v2.0.5 would fall inside v3's range.
The mapping follows semver order instead:

```
vMAJOR.MINOR.PATCH  →  (2000 + MAJOR)-01-01T00:00:00Z + MINOR days + PATCH seconds
v2.0.4              →  2002-01-01T00:00:04Z
v3.0.0              →  2003-01-01T00:00:00Z
```

A version range `[since, until)` becomes a valid-time window `[:valid-from since, :valid-to until)`.
A page at version V is whatever is valid at `:valid-at time(V)`. Semver order is total, and a
later major includes everything from earlier ones unless a range ends, so one window per
fragment covers every case so far. A fragment valid in two disjoint ranges gets one entity per
range (both point at the same blob).

The versions themselves live in `versions.toml` (tag, line, release date, released or not).

## Content sources

### Authored pages (`content/*.md`)

One file per page, with a front-matter block:

```
---
title: Datalog Reference
section: Reference
order: 20
since: v2.0.0        # optional, default: first version
until: v3.0.0        # optional, default: open
---
```

Parts that differ between versions are wrapped in annotations, each on its own line:

```
<!-- @since v3.0.0 -->     ... <!-- @end -->
<!-- @until v3.0.0 -->     ... <!-- @end -->
<!-- @range v2.0.3..v3.0.0 --> ... <!-- @end -->
```

Annotations nest (windows intersect). They are ignored inside code fences. They should wrap
whole Markdown blocks: a list split by an annotation renders as two lists.

Links between pages use the target's slug: `[negation](datalog-reference#negation)`. The builder
rewrites them to `../datalog-reference/#negation` (so they stay in the current version) and fails
the build when a link points at a page or heading that does not exist in that version. Links to the site's tools use `site:`, e.g. `[What changed](site:diff/)`.

### Snapshot pages (`snapshots.toml`)

Some pages already exist per release in the main repository (`CHANGELOG.md`,
`docs/ERROR_REFERENCE.md`). The builder reads them with `git show <ref>:<path>` for each version,
so they are never copied by hand. Each version's text is split into fragments; identical
fragments hash to the same blob, and a fragment's window covers the consecutive versions in
which it appears at the same position.

## Fragments and blobs

A page is split into fragments at `##`/`###` headings and annotation boundaries. Each fragment is
rendered to HTML once (pulldown-cmark, GitHub-style heading ids, Datalog highlighting) and
stored as `blobs/<hash>.html`, where `<hash>` is the first 16 hex digits of the SHA-256 of the
HTML. A fragment that is the same in v2 and v3 is one blob.

## The index: `docs.graph`

A Minigraf file (format v7, readable by `@minigraf/browser` 2.0.x) built natively by the builder
through Datalog `transact` statements:

| Entity | Attributes | Valid time |
|---|---|---|
| version | `:version/tag`, `:version/line`, `:version/at`, `:version/date`, `:version/released` | forever (queried with `:any-valid-time`) |
| page | `:page/slug`, `:page/title`, `:page/section`, `:page/order` | the page's window |
| fragment | `:frag/page` (slug), `:frag/order`, `:frag/blob`, `:frag/heading`, `:frag/since` | the fragment's window |

A page at a version is one query:

```
(query [:find ?order ?blob
        :valid-at "2003-01-01T00:00:00Z"
        :where [?f :frag/page "architecture"]
               [?f :frag/order ?order]
               [?f :frag/blob ?blob]])
```

The fragments that changed between two versions are those whose window starts or ends between
them, read with `:any-valid-time` and the `:db/valid-from` / `:db/valid-to` pseudo-attributes.

Each build writes the graph from scratch. Keeping transaction-time history across builds (so
`:as-of` shows the docs as published on an earlier date) is a possible follow-up.

## Output (`dist/`)

```
index.html                 → redirects to latest/
<tag>/<slug>/index.html    one page per version (v2.0.0 … v2.0.4, v3.0.0)
latest/<slug>/index.html   copy of the newest released version, canonical link to it
diff/index.html            "what changed" view (client-side)
console/index.html         Datalog console (client-side)
blobs/<hash>.html          fragment HTML
docs.graph                 the index
assets/                    CSS, JS, vendored @minigraf/browser
404.html
```

Every page is complete HTML. The version selector is a list of links, so it works without
JavaScript.

## Browser behaviour

`assets/app.js` loads the WASM module when the browser is idle, then imports `docs.graph` into a
`BrowserDb.openInMemory()` handle. It never uses the IndexedDB-backed `BrowserDb.open()`: nothing is
stored in the browser, so a page refresh always reads the graph the server has. `docs.graph` is
fetched with `cache: "no-cache"` (revalidated on every load); blobs are content-addressed, so a
changed fragment always has a new URL.

- **Version switch:** query the current page at the new version, keep fragments whose blob is
  unchanged, fetch the others, swap them in, update the sidebar and URL (`history.pushState`),
  and keep the reader's position. Changed fragments are briefly highlighted. A page that does not
  exist in the target version falls back to normal navigation.
- **Diff view:** for two versions, list pages added, removed and changed; within a changed page,
  show removed and added fragments with unchanged runs collapsed. The Datalog used is shown.
- **Console:** run Datalog against `docs.graph`, with a version picker that fills in `:valid-at`.

## Wiki retirement

After launch, each wiki page is replaced by a short stub linking to its `latest/` permalink.
GitHub wikis cannot send HTTP redirects.

## Upgrading the index format

The builder pins the `minigraf` crate to the same major version as the vendored
`@minigraf/browser`. When v3.0.0 ships (format v8), both move together.
