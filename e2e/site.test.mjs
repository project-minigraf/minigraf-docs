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
  assert.equal(await page.locator(".picker a[data-version]").count(), 6);
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
