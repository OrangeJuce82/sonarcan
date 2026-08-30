import type { ImportJobState } from "./types";

export interface ImportCompletion {
  completed: number;
  failed: number;
}

export function completedImportBatch(
  expectedJobs: number,
  states: Iterable<ImportJobState>,
): ImportCompletion | null {
  const values = [...states];
  if (
    values.length !== expectedJobs
    || values.some((state) => state !== "completed" && state !== "failed")
  ) return null;
  return {
    completed: values.filter((state) => state === "completed").length,
    failed: values.filter((state) => state === "failed").length,
  };
}
