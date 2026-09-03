import assert from "node:assert/strict";
import test from "node:test";

import { BackgroundTaskScheduler } from "./backgroundTaskScheduler.ts";

const tick = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 5));

test("keeps background work paused until foreground work clears", async () => {
  const scheduler = new BackgroundTaskScheduler({ idleDelayMs: 0, betweenTasksMs: 0 });
  const calls: string[] = [];
  scheduler.enqueue({ scope: "project", key: "one", run: async () => { calls.push("one"); } });
  await tick();
  assert.deepEqual(calls, []);

  scheduler.setBlocked(false);
  await tick();
  assert.deepEqual(calls, ["one"]);
});

test("runs queued work sequentially and deduplicates task keys", async () => {
  const scheduler = new BackgroundTaskScheduler({ idleDelayMs: 0, betweenTasksMs: 0 });
  const calls: string[] = [];
  let releaseFirst: (() => void) | undefined;
  scheduler.enqueue({
    scope: "project",
    key: "one",
    run: () => new Promise<void>((resolve) => {
      calls.push("one:start");
      releaseFirst = resolve;
    }),
  });
  scheduler.enqueue({ scope: "project", key: "one", run: async () => { calls.push("duplicate"); } });
  scheduler.enqueue({ scope: "project", key: "two", run: async () => { calls.push("two"); } });
  scheduler.setBlocked(false);
  await tick();
  assert.deepEqual(calls, ["one:start"]);

  releaseFirst?.();
  await tick();
  assert.deepEqual(calls, ["one:start", "two"]);
});

test("cancels obsolete queued work by project scope", async () => {
  const scheduler = new BackgroundTaskScheduler({ idleDelayMs: 0, betweenTasksMs: 0 });
  const calls: string[] = [];
  scheduler.enqueue({ scope: "old", key: "one", run: async () => { calls.push("old"); } });
  scheduler.enqueue({ scope: "new", key: "one", run: async () => { calls.push("new"); } });
  scheduler.cancelScope("old");
  scheduler.setBlocked(false);
  await tick();
  assert.deepEqual(calls, ["new"]);
});
