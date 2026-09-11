# Selective ExecuTorch runtime

Release and CI builders place the target-native
`sonarcan-executorch-worker` executable in this directory. The generated
binary is intentionally ignored: each supported operating system compiles it
from the pinned ExecuTorch 1.4.1 sources and the reviewed operator manifest.

Build it with:

```sh
SONARCAN_EXECUTORCH_SOURCE=/path/to/executorch npm run executorch:runtime
```

Model PTE files are verified first-use downloads and are never stored in this
Tauri resource directory.
