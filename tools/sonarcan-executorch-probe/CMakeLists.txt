cmake_minimum_required(VERSION 3.24)
project(sonarcan_executorch_probe LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT SONARCAN_EXECUTORCH_SOURCE)
  message(FATAL_ERROR "SONARCAN_EXECUTORCH_SOURCE is required")
endif()
if(NOT EXECUTORCH_SELECT_OPS_LIST)
  message(FATAL_ERROR "EXECUTORCH_SELECT_OPS_LIST is required")
endif()

set(EXECUTORCH_BUILD_DEVTOOLS OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_EXECUTOR_RUNNER OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_EXTENSION_DATA_LOADER ON CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_EXTENSION_FLAT_TENSOR ON CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_EXTENSION_RUNNER_UTIL ON CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_KERNELS_OPTIMIZED OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_PORTABLE_OPS ON CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_BUILD_XNNPACK ON CACHE BOOL "" FORCE)
set(EXECUTORCH_ENABLE_EVENT_TRACER OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_ENABLE_PROGRAM_VERIFICATION OFF CACHE BOOL "" FORCE)
set(EXECUTORCH_XNNPACK_ENABLE_KLEIDI OFF CACHE BOOL "" FORCE)

add_subdirectory(${SONARCAN_EXECUTORCH_SOURCE} executorch)

add_executable(
  sonarcan-executorch-probe
  ${SONARCAN_EXECUTORCH_SOURCE}/examples/portable/executor_runner/executor_runner.cpp
  ${SONARCAN_EXECUTORCH_SOURCE}/extension/data_loader/file_data_loader.cpp
  ${SONARCAN_EXECUTORCH_SOURCE}/runtime/executor/test/test_backend_compiler_lib.cpp
)
target_include_directories(sonarcan-executorch-probe PRIVATE ${SONARCAN_EXECUTORCH_SOURCE})
target_link_libraries(
  sonarcan-executorch-probe
  PRIVATE
    executorch
    executorch_backends
    executorch_kernels
    extension_evalue_util
    extension_flat_tensor
    extension_runner_util
    gflags
)
if(APPLE)
  target_link_options(sonarcan-executorch-probe PRIVATE -Wl,-dead_strip)
elseif(UNIX)
  target_link_options(sonarcan-executorch-probe PRIVATE -Wl,--gc-sections)
endif()
