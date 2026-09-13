export type ProjectStartupAction = "checking" | "restoreRecent" | "createTemporary";

export function projectStartupAction(recentProjectPaths: readonly string[]): ProjectStartupAction {
  return recentProjectPaths.length > 0 ? "restoreRecent" : "createTemporary";
}

export function shouldProcessProjectOpenRequest(applicationReady: boolean): boolean {
  return applicationReady;
}

export async function prepareAnalysisForStartup(
  prepareModels: () => Promise<unknown>,
  getAnalysisCapabilities: () => Promise<{ accelerated: boolean }>,
): Promise<boolean> {
  // The accelerator probe loads the Beat This! checkpoint. Repair the
  // disposable model cache before probing so a missing model is not mistaken
  // for an unavailable accelerator.
  await prepareModels();
  return (await getAnalysisCapabilities()).accelerated;
}
