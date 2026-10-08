// Doc pages: switch versions in place. The page is server-rendered; once the docs index has
// loaded, picking a version swaps only the fragments that differ instead of loading a new page.

import {
  root,
  preload,
  readyDb,
  navAt,
  fragsAt,
  versions,
  fragsQuery,
  fetchBlob,
  escapeHtml,
  inlineTitle,
} from "./docs.js";

const body = document.body;
const slug = body.dataset.page;
const latest = body.dataset.latest;
let switching = false;

function pageUrl(tag, s = slug) {
  return new URL(`${tag}/${s}/`, root);
}

/** The fragment nearest the top of the viewport, and its offset, to keep the reader's place. */
function anchorFragment() {
  for (const el of document.querySelectorAll("#frags > .frag")) {
    const r = el.getBoundingClientRect();
    if (r.bottom > 80) return { blob: el.dataset.blob, order: el.dataset.order, top: r.top };
  }
  return null;
}

function fragElement(f, html) {
  const el = document.createElement("section");
  el.className = "frag";
  el.dataset.blob = f.blob;
  el.dataset.order = f.order;
  el.innerHTML =
    (f.added ? `<span class="badge-added">Added in ${escapeHtml(f.added)}</span>` : "") + html;
  return el;
}

function renderSidebar(nav, tag) {
  const sidebar = document.getElementById("sidebar");
  const order = [...sidebar.querySelectorAll(".nav-section h2")].map((h) => h.textContent);
  for (const e of nav) if (!order.includes(e.section)) order.push(e.section);
  const dir = tag;
  sidebar.innerHTML = order
    .map((section) => {
      const entries = nav.filter((e) => e.section === section);
      if (!entries.length) return "";
      const items = entries
        .map(
          (e) =>
            `<li><a href="${pageUrl(dir, e.slug)}" data-slug="${escapeHtml(e.slug)}"` +
            `${e.slug === slug ? ' aria-current="page"' : ""}>${inlineTitle(e.nav)}</a></li>`,
        )
        .join("");
      return `<div class="nav-section"><h2>${escapeHtml(section)}</h2><ul>${items}</ul></div>`;
    })
    .join("");
}

function renderBanner(tag, info) {
  const banner = document.getElementById("banner");
  const latestLink = `<a href="${pageUrl(latest)}">${escapeHtml(latest)}</a>`;
  if (!info.released) {
    banner.className = "banner banner-dev";
    banner.innerHTML = `You are reading the docs for <strong>${escapeHtml(tag)}</strong>, which is not released yet. The latest release is ${latestLink}.`;
    banner.hidden = false;
  } else if (tag !== latest) {
    banner.className = "banner banner-old";
    banner.innerHTML = `You are reading the docs for <strong>${escapeHtml(tag)}</strong>. The latest release is ${latestLink}.`;
    banner.hidden = false;
  } else {
    banner.hidden = true;
  }
}

function updateChrome(tag, all, title) {
  body.dataset.version = tag;
  const picker = document.getElementById("version-picker");
  picker.open = false;
  for (const a of picker.querySelectorAll("a[data-version]")) {
    if (a.dataset.version === tag) a.setAttribute("aria-current", "true");
    else a.removeAttribute("aria-current");
  }
  const current = picker.querySelector(`a[data-version="${tag}"]`);
  picker.querySelector(".picker-current").textContent = current.textContent;

  const idx = all.findIndex((v) => v.tag === tag);
  const prev = idx > 0 ? all[idx - 1].tag : tag;
  const links = document.querySelectorAll(".toplinks a");
  links[0].href = new URL(`diff/?from=${prev}&to=${tag}`, root);
  links[1].href = new URL(`console/?version=${tag}`, root);
  document.querySelector(".brand").href = new URL(`${tag}/`, root);
  const canonical = document.querySelector('link[rel="canonical"]');
  if (canonical) {
    canonical.href = tag === latest ? new URL(`latest/${slug}/`, root) : pageUrl(tag);
  }
  document.title = `${title.replace(/`/g, "")} · Minigraf ${tag}`;
}

function flash(message) {
  let toast = document.getElementById("toast");
  if (!toast) {
    toast = document.createElement("div");
    toast.id = "toast";
    toast.className = "toast";
    toast.setAttribute("role", "status");
    document.body.appendChild(toast);
  }
  toast.textContent = message;
  toast.classList.add("show");
  clearTimeout(flash.timer);
  flash.timer = setTimeout(() => toast.classList.remove("show"), 3200);
}

/**
 * Shows this page at `tag` without a page load. Returns false when it cannot (the page does not
 * exist in that version), so the caller can navigate normally.
 */
async function switchVersion(db, tag, { push = true } = {}) {
  const [nav, all] = await Promise.all([navAt(db, tag), versions(db)]);
  const entry = nav.find((e) => e.slug === slug);
  const info = all.find((v) => v.tag === tag);
  if (!entry || !info) return false;

  const frags = await fragsAt(db, slug, tag);
  const htmls = await Promise.all(frags.map((f) => fetchBlob(f.blob)));
  const container = document.getElementById("frags");
  // A page can hold the same blob twice (identical fragments), so keep a queue per blob.
  const existing = new Map();
  for (const el of container.querySelectorAll(":scope > .frag")) {
    if (!existing.has(el.dataset.blob)) existing.set(el.dataset.blob, []);
    existing.get(el.dataset.blob).push(el);
  }
  const before = container.querySelectorAll(":scope > .frag").length;
  const keep = anchorFragment();

  let changed = 0;
  const next = frags.map((f, i) => {
    const queue = existing.get(f.blob);
    const el = queue?.[0];
    if (el && (el.querySelector(":scope > .badge-added")?.textContent ?? null) ===
        (f.added ? `Added in ${f.added}` : null)) {
      queue.shift();
      el.classList.remove("frag-changed");
      el.dataset.order = f.order;
      return el;
    }
    changed++;
    const fresh = fragElement(f, htmls[i]);
    fresh.classList.add("frag-changed");
    return fresh;
  });
  const removed = before - (next.length - changed);
  container.replaceChildren(...next);

  // Keep the reader's place: put the same fragment back at the same height.
  if (keep) {
    const same = container.querySelector(`:scope > .frag[data-blob="${keep.blob}"]`);
    if (same) window.scrollBy(0, same.getBoundingClientRect().top - keep.top);
  }

  document.getElementById("page-title").innerHTML = inlineTitle(entry.title);
  renderSidebar(nav, tag);
  renderBanner(tag, info);
  updateChrome(tag, all, entry.title);
  document.getElementById("assembled-query").textContent = fragsQuery(slug, tag);
  document.getElementById("frag-count").textContent = String(frags.length);
  const consoleLink = document.querySelector(".assembled a[href*='console']");
  if (consoleLink) consoleLink.href = new URL(`console/?version=${tag}&page=${slug}`, root);

  if (push) history.pushState({ tag }, "", pageUrl(tag).href + location.hash);
  flash(
    changed || removed
      ? `${tag}: ${changed} section${changed === 1 ? "" : "s"} changed or added, ${removed} removed`
      : `${tag}: this page is the same as in the previous version`,
  );
  return true;
}

document.getElementById("version-picker")?.addEventListener("click", async (e) => {
  const a = e.target.closest("a[data-version]");
  if (!a || e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return;
  const db = readyDb();
  if (!db || switching) return; // Not loaded yet: ordinary navigation.
  const tag = a.dataset.version;
  if (tag === body.dataset.version) {
    e.preventDefault();
    document.getElementById("version-picker").open = false;
    return;
  }
  e.preventDefault();
  switching = true;
  try {
    if (!(await switchVersion(db, tag))) location.href = a.href;
  } catch (err) {
    console.warn("in-place switch failed, loading the page instead:", err);
    location.href = a.href;
  } finally {
    switching = false;
  }
});

addEventListener("popstate", async () => {
  const m = new RegExp(`/(v\\d+\\.\\d+\\.\\d+)/${slug}/?$`).exec(location.pathname);
  const db = readyDb();
  if (!m || !db) {
    location.reload();
    return;
  }
  if (m[1] === body.dataset.version) return;
  if (!(await switchVersion(db, m[1], { push: false }))) location.reload();
});

history.replaceState({ tag: body.dataset.version }, "");
preload();
