export type ProjectStartupAction = "checking" | "restoreRecent" | "createTemporary";

export function projectStartupAction(recentProjectPaths: readonly string[]): ProjectStartupAction {
  return recentProjectPaths.length > 0 ? "restoreRecent" : "createTemporary";
}

export function shouldProcessProjectOpenRequest(applicationReady: boolean): boolean {
  return applicationReady;
}
