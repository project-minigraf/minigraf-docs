// Site search. The index is search-<build id>.graph, loaded on first use; the build id comes from
// the docs.graph this page already loaded, so the two always belong to the same build. Results
// are scoped to the version being read by joining with docs.graph at :valid-at.
// See docs/specs/2026-10-12-search-design.md.

import {
  root,
  getDb,
  query,
  navAt,
  fetchGraph,
  openGraph,
  fetchBlob,
  escapeHtml,
  inlineTitle,
  versionTime,
  ednStr,
} from "./docs.js";
import { tokenize } from "./tokenize.js";

const MAX_RESULTS = 20;
const SNIPPET_CHARS = 160;
const DEBOUNCE_MS = 120;
const UPDATED =
  "The docs were updated since this page loaded. Reload the page to search.";

const box = document.querySelector(".search");
const input = document.getElementById("search-input");
const list = document.getElementById("search-results");
const status = document.getElementById("search-status");
const toggle = document.querySelector(".search-toggle");

// ── The index ────────────────────────────────────────────────────────────────

let indexPromise = null;

/** Loads the search graph once: { docs, search, anchors }. Rejects if it cannot be paired. */
function loadIndex() {
  if (!indexPromise) {
    indexPromise = (async () => {
      const docs = await getDb();
      const ids = (await query(docs, "(query [:find ?id :any-valid-time :where [?b :build/id ?id]])")).rows;
      if (ids.length !== 1) throw new Error("docs.graph has no build id");
      const id = ids[0][0];
      const search = await openGraph(await fetchGraph(`search-${id}.graph`));
      const own = (await query(search, "(query [:find ?id :any-valid-time :where [?b :build/id ?id]])")).rows;
      if (own.length !== 1 || own[0][0] !== id) throw new Error("search graph build id differs");
      const anchors = new Map(
        (await query(
          search,
          "(query [:find ?h ?a :any-valid-time :where [?b :blob/hash ?h] [?b :blob/anchor ?a]])",
        )).rows,
      );
      return { docs, search, anchors };
    })();
    indexPromise.catch(() => {}); // Reported where it is awaited.
  }
  return indexPromise;
}

const fragsByTag = new Map();

/** blob → [{ slug, order, heading }] for the fragments at a version, and the page order. */
function fragsAt(docs, tag) {
  if (!fragsByTag.has(tag)) {
    fragsByTag.set(
      tag,
      (async () => {
        const [{ rows }, nav] = await Promise.all([
          query(
            docs,
            `(query [:find ?slug ?order ?blob ?heading :valid-at "${versionTime(tag)}" ` +
              `:where [?f :frag/blob ?blob] [?f :frag/page ?slug] [?f :frag/order ?order] ` +
              `[?f :frag/heading ?heading]])`,
          ),
          navAt(docs, tag),
        ]);
        const byBlob = new Map();
        for (const [slug, order, blob, heading] of rows) {
          if (!byBlob.has(blob)) byBlob.set(blob, []);
          byBlob.get(blob).push({ slug, order, heading });
        }
        const pages = new Map(nav.map((e, i) => [e.slug, { rank: i, title: e.title }]));
        return { byBlob, pages };
      })(),
    );
  }
  return fragsByTag.get(tag);
}

/**
 * The tokens of a query, each { token, prefix }. The last is a prefix while it is being typed.
 * Parts of a hyphenated token are left out.
 */
export function parseQuery(q) {
  const tokens = tokenize(q);
  const tail = /[A-Za-z0-9-]*$/.exec(q)[0];
  const typing = /[A-Za-z0-9]$/.test(q) && tokenize(tail).length > 0;
  // A hyphenated token implies its parts, which would otherwise inflate scores (`prs-015` is
  // in fewer places than `prs`).
  const implied = new Set(tokens.filter((t) => t.includes("-")).flatMap((t) => t.split("-")));
  const seen = new Set();
  const out = [];
  tokens.forEach((token, i) => {
    if (implied.has(token)) return;
    const prefix = typing && i === tokens.length - 1;
    const key = `${prefix ? "^" : "="}${token}`;
    if (!seen.has(key)) {
      seen.add(key);
      out.push({ token, prefix });
    }
  });
  return out;
}

/** blob → weight for one token. */
async function postings(search, { token, prefix }) {
  const where = prefix
    ? `[?t :term/text ?x] [(starts-with? ?x ${ednStr(token)})] [?t :term/blobs ?blobs]`
    : `[?t :term/text ${ednStr(token)}] [?t :term/blobs ?blobs]`;
  const { rows } = await query(
    search,
    `(query [:find ?t ?blobs :any-valid-time :where ${where}])`,
  );
  const weights = new Map();
  for (const [, blobs] of rows) {
    for (const entry of blobs.split(" ")) {
      const i = entry.lastIndexOf(":");
      const blob = entry.slice(0, i);
      weights.set(blob, (weights.get(blob) ?? 0) + Number(entry.slice(i + 1)));
    }
  }
  return weights;
}

/** Results for a query at a version, best first. */
async function search(q, tag) {
  const terms = parseQuery(q);
  if (!terms.length) return { terms, results: [] };
  const index = await loadIndex();
  const [per, frags] = await Promise.all([
    Promise.all(terms.map((t) => postings(index.search, t))),
    fragsAt(index.docs, tag),
  ]);
  const results = [];
  for (const [blob, first] of per[0]) {
    let score = first;
    if (!per.every((w) => w.has(blob))) continue;
    for (const w of per.slice(1)) score += w.get(blob);
    // One result per page the blob is on, at its first position there.
    const onPages = new Map();
    for (const f of frags.byBlob.get(blob) ?? []) {
      const prev = onPages.get(f.slug);
      if (!prev || f.order < prev.order) onPages.set(f.slug, f);
    }
    for (const f of onPages.values()) {
      const page = frags.pages.get(f.slug);
      if (!page) continue;
      results.push({ blob, score, ...f, rank: page.rank, title: page.title });
    }
  }
  results.sort(
    (a, b) =>
      b.score - a.score ||
      a.rank - b.rank ||
      (a.slug < b.slug ? -1 : a.slug > b.slug ? 1 : 0) ||
      a.order - b.order,
  );
  for (const r of results) r.anchor = index.anchors.get(r.blob) ?? "";
  return { terms, results: results.slice(0, MAX_RESULTS), total: results.length };
}

// ── Snippets ────────────────────────────────────────────────────────────────

function matcher(terms) {
  const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const alts = terms.map(({ token, prefix }) =>
    prefix ? `${esc(token)}[A-Za-z0-9]*` : `${esc(token)}(?![A-Za-z0-9])`,
  );
  return new RegExp(`(?<![A-Za-z0-9])(?:${alts.join("|")})`, "gi");
}

async function snippet(blob, terms) {
  const html = await fetchBlob(blob);
  const tpl = document.createElement("template");
  tpl.innerHTML = html;
  for (const a of tpl.content.querySelectorAll(".anchor")) a.remove();
  for (const h of tpl.content.querySelectorAll("h1, h2, h3, h4, h5, h6")) h.remove();
  // Keep words in neighbouring cells, items and blocks apart.
  for (const el of tpl.content.querySelectorAll("p, li, td, th, tr, pre, dt, dd, br, blockquote")) {
    el.after(" ");
  }
  const text = tpl.content.textContent.replace(/\s+/g, " ").trim();
  // Centre on the longest token that occurs (`prs-015` before `prs`).
  let at = 0;
  for (const t of [...terms].sort((a, b) => b.token.length - a.token.length)) {
    const m = matcher([t]).exec(text);
    if (m) {
      at = m.index;
      break;
    }
  }
  const re = matcher(terms);
  let start = Math.max(0, at - SNIPPET_CHARS / 3);
  if (start > 0) start = text.indexOf(" ", start) + 1 || start;
  let piece = text.slice(start, start + SNIPPET_CHARS);
  if (start + SNIPPET_CHARS < text.length) piece = piece.replace(/\s\S*$/, "") + "…";
  if (start > 0) piece = "…" + piece;
  let out = "";
  let last = 0;
  for (const hit of piece.matchAll(re)) {
    out += escapeHtml(piece.slice(last, hit.index)) + `<mark>${escapeHtml(hit[0])}</mark>`;
    last = hit.index + hit[0].length;
  }
  return out + escapeHtml(piece.slice(last));
}

// ── UI ──────────────────────────────────────────────────────────────────────

let selected = -1;
let seq = 0;
let timer = null;
let message = null;

function tag() {
  return document.body.dataset.version;
}

function setLabel() {
  input.setAttribute("aria-label", `Search the docs for ${tag()}`);
}

function options() {
  return [...list.querySelectorAll('[role="option"]')];
}

function select(i) {
  const opts = options();
  selected = opts.length ? Math.max(-1, Math.min(i, opts.length - 1)) : -1;
  opts.forEach((o, k) => o.setAttribute("aria-selected", String(k === selected)));
  if (selected >= 0) {
    input.setAttribute("aria-activedescendant", opts[selected].id);
    opts[selected].scrollIntoView({ block: "nearest" });
  } else {
    input.removeAttribute("aria-activedescendant");
  }
}

function showMessage(text) {
  if (!message) {
    message = document.createElement("p");
    message.className = "search-message";
    box.appendChild(message);
  }
  message.textContent = text;
  message.hidden = !text;
}

function open(isOpen) {
  list.hidden = !isOpen || !list.children.length;
  input.setAttribute("aria-expanded", String(!list.hidden));
  if (!isOpen) {
    select(-1);
    showMessage("");
  }
}

function announce(text) {
  status.textContent = text;
}

function render(found, tagNow) {
  const { terms, results, total } = found;
  list.innerHTML = results
    .map(
      (r, i) =>
        `<li role="option" id="search-opt-${i}" aria-selected="false" class="search-result" ` +
        `data-href="${escapeHtml(new URL(`${tagNow}/${r.slug}/${r.anchor ? `#${r.anchor}` : ""}`, root).href)}">` +
        `<span class="search-title">${inlineTitle(r.title)}` +
        (r.heading ? ` <span class="search-heading">› ${escapeHtml(r.heading)}</span>` : "") +
        `</span><span class="search-snippet" data-blob="${escapeHtml(r.blob)}"></span></li>`,
    )
    .join("");
  selected = -1;
  open(true);
  showMessage(results.length ? "" : "No results");
  announce(
    results.length
      ? `${total} result${total === 1 ? "" : "s"}${total > results.length ? `, showing ${results.length}` : ""}`
      : "No results",
  );
  const mine = seq;
  for (const el of list.querySelectorAll(".search-snippet")) {
    snippet(el.dataset.blob, terms)
      .then((html) => {
        if (mine === seq) el.innerHTML = html;
      })
      .catch(() => {});
  }
}

async function run() {
  const q = input.value;
  const mine = ++seq;
  setLabel();
  if (!parseQuery(q).length) {
    list.innerHTML = "";
    open(false);
    announce("");
    return;
  }
  const tagNow = tag();
  let found;
  try {
    found = await search(q, tagNow);
  } catch (err) {
    console.warn("search unavailable:", err);
    if (mine !== seq) return;
    list.innerHTML = "";
    open(false);
    showMessage(UPDATED);
    announce(UPDATED);
    return;
  }
  if (mine === seq) render(found, tagNow);
}

function go(i) {
  const opt = options()[i];
  if (opt) location.href = opt.dataset.href;
}

function focusSearch() {
  box.classList.add("search-expanded");
  toggle.setAttribute("aria-expanded", "true");
  input.focus();
}

function collapse() {
  box.classList.remove("search-expanded");
  toggle.setAttribute("aria-expanded", "false");
}

function onFocus() {
  setLabel();
  if (!indexPromise) {
    announce("Loading the search index…");
    loadIndex().then(
      () => {
        if (status.textContent === "Loading the search index…") announce("");
      },
      () => {},
    );
  }
  if (list.children.length) open(true);
}

function isEditable(el) {
  return (
    el instanceof HTMLInputElement ||
    el instanceof HTMLTextAreaElement ||
    el instanceof HTMLSelectElement ||
    el?.isContentEditable
  );
}

if (box && input && list && status && toggle) {
  box.hidden = false;
  input.addEventListener("focus", onFocus);
  input.addEventListener("input", () => {
    clearTimeout(timer);
    timer = setTimeout(run, DEBOUNCE_MS);
  });
  input.addEventListener("keydown", (e) => {
    const n = options().length;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (list.hidden) open(true);
      select(selected + 1 >= n ? 0 : selected + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      select(selected <= 0 ? n - 1 : selected - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      go(selected >= 0 ? selected : 0);
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (!list.hidden) open(false);
      else {
        input.value = "";
        list.innerHTML = "";
        announce("");
        collapse();
      }
    }
  });
  // Keep focus in the field while clicking an option.
  list.addEventListener("mousedown", (e) => e.preventDefault());
  list.addEventListener("click", (e) => {
    const opt = e.target.closest('[role="option"]');
    if (opt) go(options().indexOf(opt));
  });
  list.addEventListener("mousemove", (e) => {
    const opt = e.target.closest('[role="option"]');
    if (opt) select(options().indexOf(opt));
  });
  box.addEventListener("focusout", (e) => {
    if (!box.contains(e.relatedTarget)) {
      open(false);
      if (!input.value) collapse();
    }
  });
  toggle.addEventListener("click", () => {
    if (box.classList.contains("search-expanded")) collapse();
    else focusSearch();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key !== "/" || e.metaKey || e.ctrlKey || e.altKey) return;
    if (isEditable(document.activeElement)) return;
    e.preventDefault();
    focusSearch();
  });
}
