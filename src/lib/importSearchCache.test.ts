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

  assert.equal(await cache.resolve("  Beatles   Imagine ", 1), result);
  assert.equal(cache.peek("BEATLES IMAGINE"), result);
  assert.equal(await cache.resolve("beatles imagine", 1), result);
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

  const first = cache.resolve("new query", 1);
  const second = cache.resolve("NEW QUERY", 1);
  await assert.rejects(first, /temporary/);
  await assert.rejects(second, /temporary/);
  fail = false;
  assert.equal(await cache.resolve("new query", 2), result);
  assert.equal(calls, 2);
});

test("a replacement generation never reuses a cancelled in-flight search", async () => {
  const releases: Array<(value: ImportCandidate[]) => void> = [];
  let calls = 0;
  const cache = new ImportSearchCache(() => {
    calls += 1;
    return new Promise<ImportCandidate[]>((resolve) => releases.push(resolve));
  });

  const obsolete = cache.resolve("same query", 1);
  const replacement = cache.resolve("same query", 2);
  assert.equal(calls, 2);
  releases[0](result);
  await obsolete;
  const sharedReplacement = cache.resolve("same query", 2);
  assert.equal(calls, 2);
  releases[1](result);
  assert.equal(await replacement, result);
  assert.equal(await sharedReplacement, result);
});
