import test from "node:test";
import assert from "node:assert/strict";
import { activeResourceLeds, formatResourceMemory, resourceLoad, resourcePressure } from "./resourceMetrics.ts";

test("resource load follows the busiest available resource", () => {
  assert.equal(resourceLoad({ cpuPercent: 18, gpuPercent: 73, memoryMegabytes: 11_400, memoryPercent: 46 }), 73);
  assert.equal(resourcePressure(64), "normal");
  assert.equal(resourcePressure(65), "warm");
  assert.equal(resourcePressure(85), "hot");
});

test("resource LED and memory presentation remain bounded", () => {
  assert.equal(activeResourceLeds(50, 14), 7);
  assert.equal(activeResourceLeds(200, 14), 14);
  assert.equal(formatResourceMemory(22), "22 MB");
  assert.equal(formatResourceMemory(11_400), "11.1 GB");
});
