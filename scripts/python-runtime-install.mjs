/**
 * @param {NodeJS.Platform} platform
 */
export function runtimePipArguments(platform) {
  return platform !== "darwin"
    ? ["--torch-backend", "cpu"]
    : [];
}

export const madmomBuildDependencies = [
  "setuptools==80.9.0",
  "numpy==2.3.5",
  "cython @ git+https://github.com/cython/cython.git@8a1b3c10260fa9f9a91475819d737bce59b1a3d0",
];
