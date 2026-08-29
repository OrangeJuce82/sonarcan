import assert from "node:assert/strict";
import test from "node:test";

import { ImportSearchCache } from "./importSearchCache.ts";
import type { ImportCandidate } from "./types.ts";

const result: ImportCandidate[] = [
  { input: "https://youtu.be/result", title: "Result", detail: "Channel", kind: "video" },
];

test("reuses a search when only case, spacing, or line position changes", async () => {
  let calls = 0;
  const cache = new ImportSearchCache(async () => { calls += 1; return result; });

  assert.equal(await cache.resolve("  Beatles   Imagine "), result);
  assert.equal(cache.peek("BEATLES IMAGINE"), result);
  assert.equal(await cache.resolve("beatles imagine"), result);
  assert.equal(calls, 1);
});

test("shares an in-flight search and retries failures", async () => {
  let calls = 0;
  let fail = true;
  const cache = new ImportSearchCache(async () => {
    calls += 1;
    if (fail) throw new Error("temporary");
    return result;
  });

  const first = cache.resolve("new query");
  const second = cache.resolve("NEW QUERY");
  await assert.rejects(first, /temporary/);
  await assert.rejects(second, /temporary/);
  fail = false;
  assert.equal(await cache.resolve("new query"), result);
  assert.equal(calls, 2);
});
