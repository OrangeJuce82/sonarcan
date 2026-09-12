cmake_minimum_required(VERSION 3.24)
project(sonarcan_executorch_worker LANGUAGES C CXX)

if(APPLE)
  enable_language(OBJCXX)
  set_source_files_properties(main.cpp PROPERTIES LANGUAGE OBJCXX)
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

option(SONARCAN_EXECUTORCH_ENABLE_CUDA "Build the Linux NVIDIA CUDA delegate" OFF)
option(SONARCAN_EXECUTORCH_ENABLE_MLX "Build the Apple MLX delegate" ON)

if(NOT SONARCAN_EXECUTORCH_SOURCE)
  message(FATAL_ERROR "SONARCAN_EXECUTORCH_SOURCE is required")
endif()

set(EXECUTORCH_BUILD_DEVTOOLS OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_EXECUTOR_RUNNER OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_EXTENSION_DATA_LOADER ON CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_EXTENSION_FLAT_TENSOR ON CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_EXTENSION_MODULE ON CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_EXTENSION_NAMED_DATA_MAP ON CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_EXTENSION_TENSOR ON CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_KERNELS_OPTIMIZED OFF CACHE BOOL "" FORCE)
# ExecuTorch's backend registry links this archive even when the selected
# operator list is empty. Keeping the empty archive does not expose a CPU model
# backend; production PTEs are verified to contain only delegate calls.
set(EXECUTORCH_BUILD_PORTABLE_OPS ON CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_XNNPACK OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_ENABLE_EVENT_TRACER OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_ENABLE_LOGGING ON CACHE BOOL "" FORCE)
set(EXECUTORCH_ENABLE_PROGRAM_VERIFICATION OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_XNNPACK_ENABLE_KLEIDI OFF CACHE BOOL "" FORCE)

if(SONARCAN_EXECUTORCH_ENABLE_CUDA)
  if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
    message(FATAL_ERROR "The SonArcan CUDA runtime is supported only on Linux")
  endif()
  set(EXECUTORCH_BUILD_CUDA ON CACHE BOOL "" FORCE)
endif()

# A delegated PTE cannot execute unless its backend is linked into the worker.
# Apple Silicon release builds register MLX only. Linux release builds register
# CUDA only and require its compiler plus build-time libtorch/AOTI tooling.
if(APPLE AND CMAKE_SYSTEM_PROCESSOR MATCHES "^(arm64|aarch64)$")
  set(EXECUTORCH_BUILD_COREML OFF CACHE BOOL "" FORCE)
  set(EXECUTORCH_BUILD_MLX ${SONARCAN_EXECUTORCH_ENABLE_MLX} CACHE BOOL "" FORCE)
  set(EXECUTORCH_BUILD_METAL OFF CACHE BOOL "" FORCE)
endif()

add_subdirectory(${SONARCAN_EXECUTORCH_SOURCE} executorch)

add_executable(sonarcan-executorch-worker main.cpp)
target_include_directories(
  sonarcan-executorch-worker PRIVATE ${SONARCAN_EXECUTORCH_SOURCE}
)
target_link_libraries(
  sonarcan-executorch-worker
  PRIVATE
    executorch
    executorch_backends
    extension_module_static
    extension_tensor
)
if(TARGET executorch_kernels)
  target_link_libraries(sonarcan-executorch-worker PRIVATE executorch_kernels)
endif()
if(TARGET mlxdelegate)
  target_compile_definitions(sonarcan-executorch-worker PRIVATE SONARCAN_HAS_MLX=1)
endif()
if(TARGET aoti_cuda_backend)
  target_compile_definitions(sonarcan-executorch-worker PRIVATE SONARCAN_HAS_CUDA=1)
endif()
# `executorch_backends` supplies the platform-specific whole-archive flags in
# dependency order. Adding a second force-load here moves the Apple delegate's
# static initializer ahead of the ExecuTorch registry and silently loses its
# registration.
target_compile_options(
  sonarcan-executorch-worker PRIVATE -Wall -Wextra -Werror -ffunction-sections
                                     -fdata-sections
                                     -Wno-unused-parameter
                                     -Wno-sign-compare
)
# MLX is force-loaded by `executorch_backends`; dead stripping can
# therefore remove unused kernels without removing delegate registration.
if(APPLE)
  target_link_options(sonarcan-executorch-worker PRIVATE -Wl,-dead_strip)
elseif(UNIX)
  target_link_options(
    sonarcan-executorch-worker PRIVATE -Wl,--gc-sections -Wl,-rpath,$ORIGIN
  )
endif()
