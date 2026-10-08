// Shared by every page: the in-memory docs index, the queries the builder also runs, and the
// page chrome (theme, sidebar). The database is always in memory (BrowserDb.openInMemory) and
// loaded from the server's docs.graph, so a refresh always shows what the server has.

import init, { BrowserDb } from "./vendor/minigraf-browser/minigraf_wasm.js";

export const root = new URL(
  document.querySelector('meta[name="docs-root"]').content,
  location.href,
);

// ── Versions ─────────────────────────────────────────────────────────────────

/** Version time: vMAJOR.MINOR.PATCH → (2000+MAJOR)-01-01 + MINOR days + PATCH seconds. */
export function versionMillis(tag) {
  const m = /^v(\d+)\.(\d+)\.(\d+)$/.exec(tag);
  if (!m) throw new Error(`not a version: ${tag}`);
  return Date.UTC(2000 + Number(m[1]), 0, 1 + Number(m[2]), 0, 0, Number(m[3]));
}

export function versionTime(tag) {
  return new Date(versionMillis(tag)).toISOString().replace(/\.\d{3}Z$/, "Z");
}

export function compareVersions(a, b) {
  return versionMillis(a) - versionMillis(b);
}

// ── Queries (same text as the builder's, see src/graph.rs) ──────────────────

export function ednStr(s) {
  return `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\n/g, " ")}"`;
}

export function navQuery(tag) {
  return (
    `(query [:find ?slug ?title ?nav ?section ?order :valid-at "${versionTime(tag)}" ` +
    `:where [?p :page/slug ?slug] [?p :page/title ?title] [?p :page/nav ?nav] ` +
    `[?p :page/section ?section] [?p :page/order ?order]])`
  );
}

export function fragsQuery(slug, tag) {
  return (
    `(query [:find ?order ?blob ?added :valid-at "${versionTime(tag)}" ` +
    `:where [?f :frag/page ${ednStr(slug)}] [?f :frag/order ?order] [?f :frag/blob ?blob] ` +
    `[?f :frag/added-in ?added]])`
  );
}

export const versionsQuery =
  "(query [:find ?tag ?line ?date ?released :any-valid-time " +
  ":where [?v :version/tag ?tag] [?v :version/line ?line] [?v :version/date ?date] " +
  "[?v :version/released ?released]])";

// ── The database ─────────────────────────────────────────────────────────────

async function fetchGraph() {
  if ("DecompressionStream" in window) {
    try {
      const res = await fetch(new URL("docs.graph.gz", root), { cache: "no-cache" });
      if (res.ok) {
        const stream = res.body.pipeThrough(new DecompressionStream("gzip"));
        return new Uint8Array(await new Response(stream).arrayBuffer());
      }
    } catch {
      // Fall through to the uncompressed file.
    }
  }
  const res = await fetch(new URL("docs.graph", root), { cache: "no-cache" });
  if (!res.ok) throw new Error(`docs.graph: HTTP ${res.status}`);
  return new Uint8Array(await res.arrayBuffer());
}

let dbPromise = null;
let dbReady = null;

/** The docs index, loaded once per page. */
export function getDb() {
  if (!dbPromise) {
    dbPromise = (async () => {
      await init({
        module_or_path: new URL("vendor/minigraf-browser/minigraf_wasm_bg.wasm", import.meta.url),
      });
      const db = BrowserDb.openInMemory();
      await db.importGraph(await fetchGraph());
      dbReady = db;
      document.documentElement.dataset.index = "ready";
      return db;
    })();
  }
  return dbPromise;
}

/** The database if it has finished loading, else null. */
export function readyDb() {
  return dbReady;
}

export async function query(db, datalog) {
  const out = JSON.parse(await db.execute(datalog));
  return { vars: out.variables ?? [], rows: out.results ?? [], raw: out };
}

export async function navAt(db, tag) {
  const { rows } = await query(db, navQuery(tag));
  return rows
    .map(([slug, title, nav, section, order]) => ({ slug, title, nav, section, order }))
    .sort((a, b) => a.order - b.order || a.slug.localeCompare(b.slug));
}

export async function fragsAt(db, slug, tag) {
  const { rows } = await query(db, fragsQuery(slug, tag));
  return rows
    .map(([order, blob, added]) => ({ order, blob, added: added || null }))
    .sort((a, b) => a.order - b.order);
}

export async function versions(db) {
  const { rows } = await query(db, versionsQuery);
  return rows
    .map(([tag, line, date, released]) => ({ tag, line, date, released }))
    .sort((a, b) => compareVersions(a.tag, b.tag));
}

const blobCache = new Map();

export function fetchBlob(hash) {
  if (!blobCache.has(hash)) {
    blobCache.set(
      hash,
      fetch(new URL(`blobs/${hash}.html`, root)).then((r) => {
        if (!r.ok) throw new Error(`blob ${hash}: HTTP ${r.status}`);
        return r.text();
      }),
    );
  }
  return blobCache.get(hash);
}

/** Starts loading the index when the browser is idle. */
export function preload() {
  const start = () => getDb().catch((e) => console.warn("docs index unavailable:", e));
  if ("requestIdleCallback" in window) requestIdleCallback(start, { timeout: 3000 });
  else setTimeout(start, 500);
}

// ── Page chrome ──────────────────────────────────────────────────────────────

export function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Same as the builder's inline_title: escape, and render `code` spans. */
export function inlineTitle(s) {
  return s
    .split("`")
    .map((part, i) => (i % 2 ? `<code>${escapeHtml(part)}</code>` : escapeHtml(part)))
    .join("");
}

function setupChrome() {
  const toggle = document.querySelector(".menu-toggle");
  const sidebar = document.getElementById("sidebar");
  if (toggle && sidebar) {
    toggle.addEventListener("click", () => {
      const open = document.body.classList.toggle("nav-open");
      toggle.setAttribute("aria-expanded", String(open));
    });
    sidebar.addEventListener("click", (e) => {
      if (e.target.closest("a")) document.body.classList.remove("nav-open");
    });
  }
  const theme = document.querySelector(".theme-toggle");
  if (theme) {
    theme.addEventListener("click", () => {
      const el = document.documentElement;
      const dark =
        el.dataset.theme === "dark" ||
        (!el.dataset.theme && matchMedia("(prefers-color-scheme: dark)").matches);
      el.dataset.theme = dark ? "light" : "dark";
      try {
        localStorage.setItem("theme", el.dataset.theme);
      } catch {
        // Storage may be unavailable; the toggle still works for this page.
      }
    });
  }
  // Close the version picker on outside click or Escape.
  const picker = document.getElementById("version-picker");
  if (picker) {
    document.addEventListener("click", (e) => {
      if (picker.open && !picker.contains(e.target)) picker.open = false;
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && picker.open) {
        picker.open = false;
        picker.querySelector("summary").focus();
      }
    });
  }
}

setupChrome();
