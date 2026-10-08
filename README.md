# minigraf-docs

Documentation for [Minigraf](https://github.com/project-minigraf/minigraf), for every release
from v2.0.0 on, at **https://project-minigraf.github.io/minigraf-docs/**.

The site is indexed by Minigraf itself. Each release is a point on Minigraf's valid-time axis,
and each section of each page is a fact with a valid-time window. A page at a version is a
`:valid-at` query. Pages are rendered at build time, so they work without JavaScript. In the
browser, `@minigraf/browser` loads the same index into memory to switch versions in place, show
what changed between two versions, and run Datalog in a console. [DESIGN.md](DESIGN.md) explains
how it works.

## Editing pages

Pages are Markdown files in `content/`. The file name is the page's URL slug.

```markdown
---
title: Datalog Reference
nav: Datalog reference      # sidebar label (optional)
section: Reference          # sidebar group, see site.toml
order: 10                   # position within the group
since: v3.0.0               # optional: first version with this page
until: v3.0.0               # optional: first version without it
---
Text shared by every version.

<!-- @until v3.0.0 -->
Text for versions before v3.0.0.
<!-- @end -->
<!-- @since v3.0.0 -->
Text for v3.0.0 and later.
<!-- @end -->
<!-- @range v2.0.3..v3.0.0 -->
Text for v2.0.3 up to (not including) v3.0.0.
<!-- @end -->
```

- Annotations go on their own lines and wrap whole blocks (paragraphs, lists, tables, code
  blocks). A table or list cut in half by an annotation renders as two. To change one row of a
  table, put the whole table in both variants.
- Write the text each version should say, as if the other version did not exist. Avoid "new in
  v3" or "goes live in v3" phrasing: a section that starts after the page does gets an
  "Added in" badge automatically.
- Link to another page by slug: `[negation](datalog-reference#negation)`. Link to the site's
  tools with `site:`: `[What changed](site:diff/)`. The build fails if a link names a page or
  heading that does not exist in some version that shows the link.

`CHANGELOG.md` and `docs/ERROR_REFERENCE.md` are not copied here: the build reads them from the
minigraf repository at each version's git ref (`[[snapshot]]` in `site.toml`).

## Adding a release

Add a `[[version]]` entry to `site.toml` with the tag and release date. For an unreleased
version, set `released = false` and `git_ref` to its branch (e.g. `origin/v3`); when it ships,
remove both and add the date.

## Building

```sh
cargo run -- build --source-repo ../minigraf   # writes dist/
python3 -m http.server -d dist 8000           # any static server works
```

`--source-repo` must be a clone of minigraf with its tags and branches fetched.

End-to-end tests (Chromium via Playwright) run against `dist/`:

```sh
cd e2e && npm ci && npx playwright install chromium && npm test
```

CI builds and tests every pull request, and deploys `main` to GitHub Pages. It also rebuilds
daily, so the snapshot pages follow the minigraf repository.

## Upgrading the Minigraf version

The builder (`minigraf` in `Cargo.toml`) and the vendored browser package
(`assets/vendor/minigraf-browser/`) must read the same file format. Upgrade both together; see
the vendor directory's README.

## License

MIT OR Apache-2.0, like Minigraf.
