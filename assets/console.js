// Query console: Datalog against the in-memory docs index.

import { getDb, query, versionTime, versionMillis, navQuery, fragsQuery, versionsQuery, escapeHtml } from "./docs.js";

const form = document.getElementById("console-form");
const input = document.getElementById("console-input");
const versionSel = document.getElementById("console-version");
const exampleSel = document.getElementById("console-example");
const status = document.getElementById("console-status");
const output = document.getElementById("console-output");

const params = new URLSearchParams(location.search);
const pageParam = params.get("page");
if (params.get("version") && [...versionSel.options].some((o) => o.value === params.get("version"))) {
  versionSel.value = params.get("version");
}

function pretty(q) {
  return q.replace(/ :where /, "\n        :where ").replace(/\] \[/g, "]\n               [");
}

const examples = [
  {
    label: "Pages in this version",
    build: (tag) => pretty(navQuery(tag)),
  },
  {
    label: "Fragments that make up a page",
    build: (tag) => pretty(fragsQuery(pageParam || "architecture", tag)),
  },
  {
    label: "Sections new in this version",
    build: (tag) =>
      "(query [:find ?page ?heading :any-valid-time\n" +
      "        :where [?f :frag/page ?page] [?f :frag/heading ?heading]\n" +
      "               [?f :db/valid-from ?vf]\n" +
      `               [(= ?vf ${versionMillis(tag)})]])  ; ${tag} as version time`,
  },
  {
    label: "Fragments per page",
    build: (tag) =>
      `(query [:find ?page (count ?f) :valid-at "${versionTime(tag)}"\n` +
      "        :where [?f :frag/page ?page]])",
  },
  {
    label: "Every version",
    build: () => pretty(versionsQuery),
  },
  {
    label: "Validity window of each fragment of a page",
    build: () =>
      "(query [:find ?order ?heading ?from ?to :any-valid-time\n" +
      `        :where [?f :frag/page "${pageParam || "architecture"}"] [?f :frag/order ?order]\n` +
      "               [?f :frag/heading ?heading]\n" +
      "               [?f :db/valid-from ?from] [?f :db/valid-to ?to]])",
  },
];

exampleSel.innerHTML = examples.map((e, i) => `<option value="${i}">${escapeHtml(e.label)}</option>`).join("");
if (pageParam) exampleSel.value = "1";

function loadExample() {
  input.value = examples[Number(exampleSel.value)].build(versionSel.value);
}

exampleSel.addEventListener("change", loadExample);
versionSel.addEventListener("change", () => {
  const t = versionTime(versionSel.value);
  if (/:valid-at "[^"]*"/.test(input.value)) {
    input.value = input.value.replace(/:valid-at "[^"]*"/, `:valid-at "${t}"`);
  } else {
    loadExample();
  }
});

/** Version times back to tags, for display (v0.0.0 is the floor for "from the start"). */
function describe(v) {
  if (!Number.isInteger(v) || v < 946684800000 || v >= 253402300800000) return null;
  const year = new Date(v).getUTCFullYear();
  const offset = v - Date.UTC(year, 0, 1);
  if (offset % 1000) return null;
  return `v${year - 2000}.${Math.floor(offset / 86400000)}.${(offset % 86400000) / 1000}`;
}

function cell(v) {
  if (v === null) return '<span class="nil">nil</span>';
  if (typeof v === "number" && v >= 9.2e18) return '<span class="nil">forever</span>';
  const tag = describe(v);
  const text = typeof v === "string" ? v : JSON.stringify(v);
  return escapeHtml(text) + (tag ? ` <span class="ver">${tag}</span>` : "");
}

async function run() {
  status.textContent = "Loading the docs index…";
  output.innerHTML = "";
  const db = await getDb();
  status.textContent = "Running…";
  const started = performance.now();
  try {
    const { vars, rows, raw } = await query(db, input.value);
    const ms = (performance.now() - started).toFixed(1);
    if (!raw.variables) {
      status.textContent = `Done in ${ms} ms.`;
      output.innerHTML = `<pre class="result-raw">${escapeHtml(JSON.stringify(raw))}</pre>`;
      return;
    }
    status.textContent = `${rows.length} row${rows.length === 1 ? "" : "s"} in ${ms} ms.`;
    const sorted = [...rows].sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b), undefined, { numeric: true }));
    output.innerHTML =
      `<div class="table-wrap"><table class="results"><thead><tr>${vars.map((v) => `<th>${escapeHtml(v)}</th>`).join("")}</tr></thead>` +
      `<tbody>${sorted.map((r) => `<tr>${r.map((c) => `<td>${cell(c)}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`;
  } catch (err) {
    status.textContent = "";
    output.innerHTML = `<pre class="result-error">${escapeHtml(String(err.message ?? err))}</pre>`;
  }
}

form.addEventListener("submit", (e) => {
  e.preventDefault();
  run();
});
input.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    run();
  }
});

loadExample();
run();
