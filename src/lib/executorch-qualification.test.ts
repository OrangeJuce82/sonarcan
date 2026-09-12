import assert from "node:assert/strict";
import test from "node:test";

import { verifyQualification } from "../../scripts/verify-executorch-qualification.mjs";

function qualifiedReport() {
  return {
    schemaVersion: 1,
    model: "beat-this",
    target: "macos-arm64",
    backendUsed: "mlx",
    backendEvidence: "delegate trace artifact sha256:example",
    precision: "fp32",
    referenceRevision: "python-reference@example",
    candidateRevision: "pte@example",
    resultComparison: {
      kind: "timeline",
      metric: "maximum timestamp error seconds",
      value: 0.0001,
      threshold: 0.001,
      passed: true,
    },
    measurements: {
      inferenceTimeMs: 100,
      realTimeFactor: 0.1,
      peakRssBytes: 1,
      loadTimeMs: 10,
      runtimeBytes: 1,
      programBytes: 1,
    },
    productionReady: true,
  };
}

test("qualification accepts a complete measured parity report", () => {
  assert.equal(verifyQualification(qualifiedReport()).backendUsed, "mlx");
});

test("qualification rejects an unproven backend or failed parity", () => {
  const missingEvidence = qualifiedReport();
  missingEvidence.backendEvidence = "";
  assert.throws(() => verifyQualification(missingEvidence), /backendEvidence/u);

  const drifted = qualifiedReport();
  drifted.resultComparison.value = 0.01;
  assert.throws(() => verifyQualification(drifted), /parity/u);
});

test("qualification rejects quantized artifacts during the FP32 and FP16 phase", () => {
  const quantized = qualifiedReport();
  quantized.precision = "int8";
  assert.throws(() => verifyQualification(quantized), /fp32 or fp16/u);
});

test("qualification rejects every production CPU backend", () => {
  const cpu = qualifiedReport();
  cpu.backendUsed = "xnnpack";
  assert.throws(() => verifyQualification(cpu), /unsupported backendUsed/u);
});
