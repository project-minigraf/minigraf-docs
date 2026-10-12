// End-to-end tests against a built site in ../dist, served under /minigraf-docs/ the way
// GitHub Pages serves it (including its 404 page). Run after `cargo run -- build`.

import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const BASE = "/minigraf-docs/";
const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".wasm": "application/wasm",
  ".xml": "application/xml",
};

let server;
let browser;
let origin;

async function resolveFile(urlPath) {
  if (!urlPath.startsWith(BASE)) return null;
  let p = normalize(join(dist, decodeURIComponent(urlPath.slice(BASE.length))));
  if (!p.startsWith(dist)) return null;
  try {
    const s = await stat(p);
    if (s.isDirectory()) {
      if (!urlPath.endsWith("/")) return { redirect: `${urlPath}/` };
      p = join(p, "index.html");
      await stat(p);
    }
    return { path: p };
  } catch {
    return null;
  }
}

before(async () => {
  server = createServer(async (req, res) => {
    const url = new URL(req.url, "http://x");
    const found = await resolveFile(url.pathname);
    if (found?.redirect) {
      res.writeHead(301, { location: found.redirect + url.search });
      return res.end();
    }
    const path = found?.path ?? join(dist, "404.html");
    res.writeHead(found ? 200 : 404, {
      "content-type": TYPES[extname(path)] ?? "application/octet-stream",
    });
    res.end(await readFile(path));
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch();
});

after(async () => {
  await browser?.close();
  server?.close();
});

async function openPage(path, opts = {}) {
  const context = await browser.newContext(opts);
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  await page.goto(origin + BASE + path);
  return { page, context, errors };
}

const indexReady = (page) =>
  page.waitForFunction(() => document.documentElement.dataset.index === "ready", null, {
    timeout: 20000,
  });

test("doc pages are complete without JavaScript", async () => {
  const { page, context } = await openPage("v2.0.4/architecture/", { javaScriptEnabled: false });
  assert.equal(await page.locator("h1#page-title").textContent(), "Architecture");
  assert.ok((await page.locator("#frags > .frag").count()) > 5);
  assert.equal(await page.locator(".picker a[data-version]").count(), 7);
  assert.ok(await page.locator('.sidebar a[aria-current="page"]').count());
  // v2 documents format v7 as current; v3-only sections are absent.
  const text = await page.locator("#frags").textContent();
  assert.ok(text.includes("v7"));
  await context.close();
});

test("switching versions swaps fragments in place and back restores", async () => {
  const { page, context, errors } = await openPage("v2.0.4/architecture/");
  await indexReady(page);
  await page.evaluate(() => (window.__noReload = true));
  const before = await page.locator("#frags > .frag").evaluateAll((els) => els.map((e) => e.dataset.blob));

  await page.locator("#version-picker summary").click();
  await page.locator('#version-picker a[data-version="v3.0.0"]').click();
  await page.waitForURL(/\/v3\.0\.0\/architecture\/$/);
  assert.equal(await page.evaluate(() => window.__noReload), true, "page must not reload");
  assert.equal(await page.locator("body").getAttribute("data-version"), "v3.0.0");
  const afterSwitch = await page.locator("#frags > .frag").evaluateAll((els) => els.map((e) => e.dataset.blob));
  assert.notDeepEqual(afterSwitch, before);
  assert.ok(afterSwitch.some((b) => before.includes(b)), "shared fragments are kept");
  assert.ok(await page.locator(".banner-dev").isVisible());
  assert.match(await page.locator("#toast").textContent(), /v3\.0\.0/);
  assert.match(await page.locator("#assembled-query").textContent(), /2003-01-01T00:00:00Z/);

  await page.goBack();
  await page.waitForURL(/\/v2\.0\.4\/architecture\/$/);
  await page.waitForFunction(() => document.body.dataset.version === "v2.0.4");
  const restored = await page.locator("#frags > .frag").evaluateAll((els) => els.map((e) => e.dataset.blob));
  assert.deepEqual(restored, before);
  assert.deepEqual(errors, []);
  await context.close();
});

test("the server-rendered v3 page matches the in-place switch", async () => {
  const { page, context } = await openPage("v3.0.0/architecture/", { javaScriptEnabled: false });
  const ssr = await page.locator("#frags > .frag").evaluateAll((els) => els.map((e) => e.dataset.blob));
  await context.close();
  const live = await openPage("v2.0.4/architecture/");
  await indexReady(live.page);
  await live.page.locator("#version-picker summary").click();
  await live.page.locator('#version-picker a[data-version="v3.0.0"]').click();
  await live.page.waitForFunction(() => document.body.dataset.version === "v3.0.0");
  const swapped = await live.page.locator("#frags > .frag").evaluateAll((els) => els.map((e) => e.dataset.blob));
  assert.deepEqual(swapped, ssr);
  await live.context.close();
});

test("diff view lists what changed between v2.0.4 and v3.0.0", async () => {
  const { page, context, errors } = await openPage("diff/?from=v2.0.4&to=v3.0.0");
  await page.locator(".diff-index").waitFor({ timeout: 20000 });
  const index = await page.locator(".diff-index").textContent();
  for (const title of ["Architecture", "Changelog", "Error Reference"]) {
    assert.ok(index.includes(title), `${title} changed`);
  }
  assert.ok((await page.locator(".diff-added").count()) > 0);
  assert.match(await page.locator("#diff-query").textContent(), /:db\/valid-from/);
  assert.deepEqual(errors, []);
  await context.close();
});

test("diff view reports identical versions", async () => {
  const { page, context } = await openPage("diff/?from=v2.0.0&to=v2.0.0");
  await page.waitForFunction(() => document.getElementById("diff-status").textContent.includes("different"));
  await context.close();
});

test("query console runs Datalog and reports errors", async () => {
  const { page, context, errors } = await openPage("console/?version=v3.0.0&page=architecture");
  await page.locator("table.results").waitFor({ timeout: 20000 });
  assert.ok((await page.locator("table.results tbody tr").count()) > 5);
  await page.locator("#console-input").fill("(query [:find ?x :where [?e :a ?x]");
  await page.locator("#console-form button[type=submit]").click();
  await page.locator(".result-error").waitFor();
  assert.deepEqual(errors, []);
  await context.close();
});

test("old wiki paths and bare versions redirect", async () => {
  const { page, context } = await openPage("Datalog-Reference");
  await page.waitForURL(/\/latest\/datalog-reference\/$/);
  await page.goto(origin + BASE + "v3.0.0/");
  await page.waitForURL(/\/v3\.0\.0\/home\/$/);
  await page.goto(origin + BASE);
  await page.waitForURL(/\/latest\/home\/$/);
  await context.close();
});

test("pages fit a phone screen", async () => {
  for (const path of ["latest/datalog-reference/", "diff/?from=v2.0.4&to=v3.0.0", "console/"]) {
    const { page, context } = await openPage(path, { viewport: { width: 375, height: 760 } });
    await page.waitForLoadState("networkidle");
    // The diff renders after the docs graph loads; measure the rendered page.
    if (path.startsWith("diff/")) await page.locator(".diff-index").waitFor({ timeout: 20000 });
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth);
    assert.ok(overflow <= 0, `${path} scrolls sideways by ${overflow}px`);
    await context.close();
  }
});

// ── Search ───────────────────────────────────────────────────────────────────

/** Types a query and waits for the result count to be announced. */
async function searchFor(page, q) {
  await page.locator("#search-input").fill(q);
  await page.waitForFunction(
    () => /results?|No results|updated/.test(document.getElementById("search-status").textContent),
    null,
    { timeout: 20000 },
  );
  return page.locator('#search-results [role="option"]');
}

async function clearSearch(page) {
  await page.locator("#search-input").fill("");
  await page.waitForFunction(() => document.getElementById("search-status").textContent === "");
}

test("search is hidden without JavaScript", async () => {
  const { page, context } = await openPage("v3.0.0/home/", { javaScriptEnabled: false });
  assert.equal(await page.locator(".search").isVisible(), false);
  await context.close();
});

test("search finds terms, scoped to the version being read", async () => {
  const { page, context, errors } = await openPage("v3.0.0/home/");
  const input = page.locator("#search-input");
  assert.ok(await input.isVisible());
  assert.equal(await input.getAttribute("role"), "combobox");

  let opts = await searchFor(page, "LogWriter");
  assert.ok((await opts.count()) > 0, "LogWriter is in v3.0.0");
  assert.equal(await input.getAttribute("aria-expanded"), "true");
  const href = await opts.first().getAttribute("data-href");
  assert.match(href, /\/v3\.0\.0\/[a-z0-9-]+\/(#[a-z0-9-]+)?$/);
  await page.waitForFunction(() => document.querySelector("#search-results mark") !== null);

  await clearSearch(page);
  opts = await searchFor(page, "recurs");
  const titles = await page.locator("#search-results").textContent();
  assert.ok((await opts.count()) > 0);
  assert.match(titles.toLowerCase(), /recursive/, "prefix matches recursive");

  await clearSearch(page);
  const one = await (await searchFor(page, "negation")).count();
  await clearSearch(page);
  const both = await (await searchFor(page, "negation stratification ")).count();
  assert.ok(both > 0 && both <= one, "two terms narrow the results");

  await clearSearch(page);
  opts = await searchFor(page, "PRS-015 ");
  assert.match(await opts.first().getAttribute("data-href"), /error-reference\/#prs-015/);

  await clearSearch(page);
  assert.equal(await (await searchFor(page, "zzqqxx")).count(), 0);
  assert.equal(await page.locator(".search-message").textContent(), "No results");
  assert.deepEqual(errors, []);
  await context.close();

  const old = await openPage("v2.0.4/home/");
  assert.equal(await (await searchFor(old.page, "LogWriter")).count(), 0, "not in v2.0.4");
  assert.equal(
    await old.page.locator("#search-input").getAttribute("aria-label"),
    "Search the docs for v2.0.4",
  );
  await old.context.close();
});

test("search follows an in-place version switch", async () => {
  const { page, context } = await openPage("v3.0.0/architecture/");
  await indexReady(page);
  assert.ok((await (await searchFor(page, "LogWriter")).count()) > 0);
  await clearSearch(page);
  await page.locator("#version-picker summary").click();
  await page.locator('#version-picker a[data-version="v2.0.4"]').click();
  await page.waitForFunction(() => document.body.dataset.version === "v2.0.4");
  assert.equal(await (await searchFor(page, "LogWriter")).count(), 0);

  // Results already shown are redone for the new version.
  await clearSearch(page);
  assert.equal(await (await searchFor(page, "LogWriter")).count(), 0);
  await page.locator("#version-picker summary").click();
  await page.locator('#version-picker a[data-version="v3.0.0"]').click();
  await page.waitForFunction(
    () => document.querySelectorAll('#search-results [role="option"]').length > 0,
    null,
    { timeout: 20000 },
  );
  const href = await page.locator("#search-opt-0").getAttribute("data-href");
  assert.match(href, /\/v3\.0\.0\//);
  await context.close();
});

test("search keyboard: slash, arrows, Enter and Escape", async () => {
  const { page, context } = await openPage("v3.0.0/home/");
  await page.locator("main").click();
  await page.keyboard.press("/");
  const input = page.locator("#search-input");
  assert.equal(await page.evaluate(() => document.activeElement.id), "search-input");
  await input.pressSequentially("checkpoint");
  await page.waitForFunction(() => /results?/.test(document.getElementById("search-status").textContent));

  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  const second = page.locator("#search-opt-1");
  assert.equal(await second.getAttribute("aria-selected"), "true");
  assert.equal(await input.getAttribute("aria-activedescendant"), "search-opt-1");

  await page.keyboard.press("Escape");
  assert.equal(await page.locator("#search-results").isVisible(), false);
  assert.equal(await input.getAttribute("aria-expanded"), "false");
  assert.equal(await input.inputValue(), "checkpoint");
  await page.keyboard.press("Escape");
  assert.equal(await input.inputValue(), "");

  await input.pressSequentially("checkpoint");
  await page.waitForFunction(() => /results?/.test(document.getElementById("search-status").textContent));
  const href = await page.locator("#search-opt-0").getAttribute("data-href");
  await page.keyboard.press("Enter");
  await page.waitForURL(href);
  await context.close();
});

test("a result in a section split by @since/@until opens at its section heading", async () => {
  // The Pages list on home is split around the Architecture @until/@since blocks, so the
  // Comparison line is in a fragment that does not start with a heading.
  const { page, context, errors } = await openPage("v3.0.0/home/");
  await searchFor(page, "side-by-side comparison ");
  const opt = page.locator('#search-results [role="option"]', {
    hasText: "Side-by-side comparison with",
  });
  await opt.waitFor();
  assert.equal(await opt.count(), 1);
  assert.match(await opt.locator(".search-heading").textContent(), /Pages/);
  assert.match(await opt.getAttribute("data-href"), /\/v3\.0\.0\/home\/#pages$/);
  await opt.click();
  await page.waitForURL(/#pages$/);
  await page.waitForFunction(() => scrollY > 0);
  const top = await page.evaluate(() => document.getElementById("pages").getBoundingClientRect().top);
  assert.ok(top >= 0 && top < 200, "the Pages heading is at the top of the window");
  assert.deepEqual(errors, []);
  await context.close();
});

test("search reports a search index from another build", async () => {
  const { page, context } = await openPage("v3.0.0/home/");
  await page.route(/search-[0-9a-f]+\.graph/, (route) => route.fulfill({ status: 404, body: "" }));
  await searchFor(page, "query");
  assert.match(await page.locator(".search-message").textContent(), /Reload the page/);
  assert.match(await page.locator("#search-status").textContent(), /Reload the page/);
  await context.close();
});

test("search fits a phone screen", async () => {
  const { page, context } = await openPage("v3.0.0/home/", { viewport: { width: 375, height: 740 } });
  const toggle = page.locator(".search-toggle");
  assert.ok(await toggle.isVisible());
  assert.equal(await page.locator("#search-input").isVisible(), false);
  await toggle.click();
  assert.equal(await toggle.getAttribute("aria-expanded"), "true");
  assert.ok((await (await searchFor(page, "query")).count()) > 0);
  const width = await page.evaluate(() => document.documentElement.scrollWidth);
  assert.ok(width <= 375, `page is ${width}px wide`);
  await context.close();
});
