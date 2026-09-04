/**
 * @param {NodeJS.Platform} platform
 */
export function runtimePipArguments(platform) {
  return platform !== "darwin"
    ? ["--torch-backend", "cpu"]
    : [];
}
