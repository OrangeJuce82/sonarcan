import assert from "node:assert/strict";
import test from "node:test";

import { filterLogs, logOrigins } from "./logFilters.ts";
import type { AppLogEntry } from "./types.ts";

const entries: AppLogEntry[] = [
  { timestampMs: 1, origin: "rust", level: "debug", message: "decoded" },
  { timestampMs: 2, origin: "mlx", level: "info", message: "model ready" },
  { timestampMs: 3, origin: "rust", level: "warn", message: "slow callback" },
  { timestampMs: 4, origin: "webview", level: "error", message: "render failed" },
];

test("log filters apply a minimum severity and an optional dynamic origin", () => {
  assert.deepEqual(filterLogs(entries, "warn", null).map((entry) => entry.message), ["slow callback", "render failed"]);
  assert.deepEqual(filterLogs(entries, "debug", "rust").map((entry) => entry.message), ["decoded", "slow callback"]);
});

test("log origins are unique and stable for dynamic select options", () => {
  assert.deepEqual(logOrigins(entries), ["mlx", "rust", "webview"]);
});
