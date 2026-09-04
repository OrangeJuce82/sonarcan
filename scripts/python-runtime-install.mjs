/**
 * @param {"chord" | "stem"} runtimeName
 * @param {NodeJS.Platform} platform
 */
export function runtimePipArguments(runtimeName, platform) {
  return runtimeName === "stem" && platform !== "darwin"
    ? ["--torch-backend", "cpu"]
    : [];
}
