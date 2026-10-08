// "What changed": the fragments whose valid-time window starts or ends between two versions.

import {
  root,
  getDb,
  query,
  navAt,
  versionMillis,
  versionTime,
  ednStr,
  fetchBlob,
  escapeHtml,
  inlineTitle,
} from "./docs.js";

const form = document.getElementById("diff-form");
const fromSel = document.getElementById("diff-from");
const toSel = document.getElementById("diff-to");
const status = document.getElementById("diff-status");
const summary = document.getElementById("diff-summary");
const results = document.getElementById("diff-results");
const queriesBox = document.getElementById("diff-queries");
const queryText = document.getElementById("diff-query");

const params = new URLSearchParams(location.search);
for (const [sel, key] of [[fromSel, "from"], [toSel, "to"]]) {
  const v = params.get(key);
  if (v && [...sel.options].some((o) => o.value === v)) sel.value = v;
}

/** Clauses for "valid at version time x and not at y", on ?vf/?vt. */
function presentNotAt(x, y) {
  return y < x
    ? `[(> ?vf ${y})] [(<= ?vf ${x})] [(> ?vt ${x})]`
    : `[(<= ?vf ${x})] [(> ?vt ${x})] [(<= ?vt ${y})]`;
}

function fragChangeQuery(x, y) {
  return (
    "(query [:find ?page ?order :any-valid-time\n" +
    "        :where [?f :frag/page ?page] [?f :frag/order ?order]\n" +
    "               [?f :db/valid-from ?vf] [?f :db/valid-to ?vt]\n" +
    `               ${presentNotAt(x, y)}])`
  );
}

function pageChangeQuery(x, y) {
  return (
    "(query [:find ?slug :any-valid-time\n" +
    "        :where [?p :page/slug ?slug]\n" +
    "               [?p :db/valid-from ?vf] [?p :db/valid-to ?vt]\n" +
    `               ${presentNotAt(x, y)}])`
  );
}

function pageFragsQuery(slug, tag) {
  return (
    `(query [:find ?order ?blob ?heading :valid-at "${versionTime(tag)}" ` +
    `:where [?f :frag/page ${ednStr(slug)}] [?f :frag/order ?order] [?f :frag/blob ?blob] ` +
    `[?f :frag/heading ?heading]])`
  );
}

async function pageFrags(db, slug, tag) {
  const { rows } = await query(db, pageFragsQuery(slug, tag));
  return rows
    .map(([order, blob, heading]) => ({ order, blob, heading }))
    .sort((a, b) => a.order - b.order);
}

function pageHref(tag, slug) {
  return new URL(`${tag}/${slug}/`, root).href;
}

function headingList(frags) {
  const names = frags.map((f) => f.heading).filter(Boolean);
  if (!names.length) return "";
  return `<ul class="diff-headings">${names.map((n) => `<li>${inlineTitle(n)}</li>`).join("")}</ul>`;
}

async function renderPageDiff(db, slug, title, a, b) {
  const [fa, fb] = await Promise.all([pageFrags(db, slug, a), pageFrags(db, slug, b)]);
  const inA = new Map(fa.map((f) => [f.order, f]));
  const inB = new Map(fb.map((f) => [f.order, f]));
  const orders = [...new Set([...inA.keys(), ...inB.keys()])].sort((x, y) => x - y);

  const items = [];
  let same = [];
  const flushSame = () => {
    if (!same.length) return;
    items.push(
      `<details class="diff-same"><summary>${same.length} unchanged section${same.length === 1 ? "" : "s"}</summary>${headingList(same)}</details>`,
    );
    same = [];
  };
  let added = 0;
  let removed = 0;
  const pending = [];
  for (const o of orders) {
    const x = inA.get(o);
    const y = inB.get(o);
    if (x && y && x.blob === y.blob) {
      same.push(y);
      continue;
    }
    flushSame();
    if (x) {
      removed++;
      const i = items.push("") - 1;
      pending.push(
        fetchBlob(x.blob).then((html) => {
          items[i] = `<div class="diff-frag diff-removed"><div class="diff-label">Only in ${escapeHtml(a)}</div>${html}</div>`;
        }),
      );
    }
    if (y) {
      added++;
      const i = items.push("") - 1;
      pending.push(
        fetchBlob(y.blob).then((html) => {
          items[i] = `<div class="diff-frag diff-added"><div class="diff-label">Only in ${escapeHtml(b)}</div>${html}</div>`;
        }),
      );
    }
  }
  flushSame();
  await Promise.all(pending);
  // Fragment links are relative to a versioned page; resolve them against version b's page.
  const holder = document.createElement("div");
  holder.innerHTML = items.join("");
  for (const link of holder.querySelectorAll("a[href]")) {
    const href = link.getAttribute("href");
    if (!/^([a-z]+:|#|\/)/i.test(href)) link.href = new URL(href, pageHref(b, slug)).href;
  }
  return {
    added,
    removed,
    html:
      `<section class="diff-page" id="page-${escapeHtml(slug)}">` +
      `<h2><a href="${pageHref(b, slug)}">${inlineTitle(title)}</a>` +
      `<span class="diff-counts"><span class="plus">+${added}</span> <span class="minus">−${removed}</span></span></h2>` +
      `<div class="diff-body">${holder.innerHTML}</div></section>`,
  };
}

async function compare(a, b) {
  results.innerHTML = "";
  summary.innerHTML = "";
  if (a === b) {
    status.textContent = "Pick two different versions.";
    return;
  }
  status.textContent = "Loading the docs index…";
  const db = await getDb();
  status.textContent = "Comparing…";
  const ta = versionMillis(a);
  const tb = versionMillis(b);

  const queries = {
    addedFrags: fragChangeQuery(tb, ta),
    removedFrags: fragChangeQuery(ta, tb),
    addedPages: pageChangeQuery(tb, ta),
    removedPages: pageChangeQuery(ta, tb),
  };
  const [addF, remF, addP, remP, navA, navB] = await Promise.all([
    query(db, queries.addedFrags),
    query(db, queries.removedFrags),
    query(db, queries.addedPages),
    query(db, queries.removedPages),
    navAt(db, a),
    navAt(db, b),
  ]);
  queryText.textContent =
    `;; Fragments in ${b} but not ${a}\n${queries.addedFrags}\n\n` +
    `;; Fragments in ${a} but not ${b}\n${queries.removedFrags}\n\n` +
    `;; Pages in ${b} but not ${a}\n${queries.addedPages}\n\n` +
    `;; Pages in ${a} but not ${b}\n${queries.removedPages}`;
  queriesBox.hidden = false;

  const newPages = new Set(addP.rows.map((r) => r[0]));
  const gonePages = new Set(remP.rows.map((r) => r[0]));
  const changed = new Set(
    [...addF.rows, ...remF.rows].map((r) => r[0]).filter((s) => !newPages.has(s) && !gonePages.has(s)),
  );
  const titleOf = new Map([...navA, ...navB].map((e) => [e.slug, e.title]));
  const orderOf = (s) => navB.find((e) => e.slug === s)?.order ?? navA.find((e) => e.slug === s)?.order ?? 0;
  const sorted = [...changed].sort((x, y) => orderOf(x) - orderOf(y));

  const parts = [];
  for (const s of [...newPages]) {
    parts.push(
      `<section class="diff-page"><h2><a href="${pageHref(b, s)}">${inlineTitle(titleOf.get(s) ?? s)}</a>` +
        `<span class="diff-tag diff-tag-added">New page in ${escapeHtml(b)}</span></h2></section>`,
    );
  }
  for (const s of [...gonePages]) {
    parts.push(
      `<section class="diff-page"><h2><a href="${pageHref(a, s)}">${inlineTitle(titleOf.get(s) ?? s)}</a>` +
        `<span class="diff-tag diff-tag-removed">Not in ${escapeHtml(b)}</span></h2></section>`,
    );
  }
  const rendered = await Promise.all(sorted.map((s) => renderPageDiff(db, s, titleOf.get(s) ?? s, a, b)));
  for (const r of rendered) parts.push(r.html);

  const total = changed.size + newPages.size + gonePages.size;
  status.textContent = "";
  if (!total) {
    summary.innerHTML = `<p class="note">The docs are the same in ${escapeHtml(a)} and ${escapeHtml(b)}.</p>`;
    return;
  }
  const links = [
    ...[...newPages].map((s) => `<li><a href="${pageHref(b, s)}">${inlineTitle(titleOf.get(s) ?? s)}</a> <span class="diff-tag diff-tag-added">new</span></li>`),
    ...[...gonePages].map((s) => `<li><a href="${pageHref(a, s)}">${inlineTitle(titleOf.get(s) ?? s)}</a> <span class="diff-tag diff-tag-removed">removed</span></li>`),
    ...sorted.map((s, i) => `<li><a href="#page-${escapeHtml(s)}">${inlineTitle(titleOf.get(s) ?? s)}</a> <span class="diff-counts"><span class="plus">+${rendered[i].added}</span> <span class="minus">−${rendered[i].removed}</span></span></li>`),
  ];
  summary.innerHTML =
    `<p class="diff-lede">${total} page${total === 1 ? "" : "s"} differ between <strong>${escapeHtml(a)}</strong> and <strong>${escapeHtml(b)}</strong>.</p>` +
    `<ul class="diff-index">${links.join("")}</ul>`;
  results.innerHTML = parts.join("");
}

form.addEventListener("submit", (e) => {
  e.preventDefault();
  const url = new URL(location.href);
  url.searchParams.set("from", fromSel.value);
  url.searchParams.set("to", toSel.value);
  history.replaceState(null, "", url);
  compare(fromSel.value, toSel.value).catch((err) => {
    status.textContent = `Could not compare: ${err.message ?? err}`;
  });
});

compare(fromSel.value, toSel.value).catch((err) => {
  status.textContent = `Could not compare: ${err.message ?? err}`;
});
