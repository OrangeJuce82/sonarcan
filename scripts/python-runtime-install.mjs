/**
 * @param {NodeJS.Platform} platform
 * @param {"nvidia" | "amd" | undefined} [gpuBackend]
 */
export function runtimePipArguments(platform, gpuBackend) {
  if (gpuBackend === "nvidia") return ["--torch-backend", "cu126"];
  if (gpuBackend === "amd") return ["--index", "https://download.pytorch.org/whl/rocm7.2"];
  return platform !== "darwin" ? ["--torch-backend", "cpu"] : [];
}

export const madmomBuildDependencies = [
  "setuptools==80.9.0",
  "numpy==2.3.5",
  "cython @ git+https://github.com/cython/cython.git@8a1b3c10260fa9f9a91475819d737bce59b1a3d0",
];
