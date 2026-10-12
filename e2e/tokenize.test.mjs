// The browser tokenizer against the vectors the builder's tokenizer is tested with.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { tokenize } from "../assets/tokenize.js";

test("tokenize matches the shared vectors", async () => {
  const cases = JSON.parse(await readFile(new URL("../tests/tokenize.json", import.meta.url)));
  assert.ok(cases.length > 10);
  for (const { input, tokens } of cases) assert.deepEqual(tokenize(input), tokens, input);
});
