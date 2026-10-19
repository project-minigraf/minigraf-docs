// The page navigator ("On this page") list, built from the headings on the page.
//
// The builder renders the same list (src/site.rs: `toc_entries`, `toc_list`). Keep the two in
// step: the end-to-end test "the browser's page navigator matches the builder's on every page"
// compares them on every page of every version.

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** A heading's HTML without its anchor link: text and `code` spans only. */
function headingHtml(node) {
  let out = "";
  for (const n of node.childNodes) {
    if (n.nodeType === 3) out += escapeHtml(n.textContent);
    else if (n.nodeType !== 1 || n.classList.contains("anchor")) continue;
    else if (n.tagName === "CODE") out += `<code>${escapeHtml(n.textContent)}</code>`;
    else out += headingHtml(n);
  }
  return out;
}

/** The `<h2 id>` and `<h3 id>` headings inside `container`, in order. */
export function tocEntries(container) {
  return [...container.querySelectorAll("h2[id], h3[id]")].map((h) => ({
    level: h.tagName === "H2" ? 2 : 3,
    id: h.id,
    html: headingHtml(h).trim(),
  }));
}

/**
 * The list's `<li>` items: `##` headings, each with its `###` headings in a nested list.
 * Empty when there are fewer than two headings.
 */
export function tocList(entries) {
  if (entries.length < 2) return "";
  let html = "";
  let item = false;
  let sub = false;
  for (const e of entries) {
    const link = `<a href="#${escapeHtml(e.id)}">${e.html}</a>`;
    if (e.level === 3 && item) {
      if (!sub) html += "<ul>";
      sub = true;
      html += `<li>${link}</li>`;
      continue;
    }
    if (sub) html += "</ul>";
    sub = false;
    if (item) html += "</li>";
    html += `<li>${link}`;
    item = true;
  }
  if (sub) html += "</ul>";
  if (item) html += "</li>";
  return html;
}
