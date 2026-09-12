#include <executorch/extension/module/module.h>
#include <executorch/extension/tensor/tensor.h>
#include <executorch/runtime/backend/interface.h>
#include <executorch/runtime/platform/runtime.h>

#include <array>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <limits>
#include <memory>
#include <stdexcept>
#include <string>
#include <system_error>
#include <type_traits>
#include <utility>
#include <vector>

#include <sys/resource.h>

namespace fs = std::filesystem;
using executorch::extension::Module;
using executorch::extension::TensorPtr;
using executorch::runtime::EValue;

namespace {

constexpr std::array<char, 8> kMagic = {'S', 'A', 'C', 'T', 'E', 'N', '0', '1'};
constexpr std::uint32_t kMaxRank = 8;
constexpr std::size_t kMaxInputs = 16;
constexpr std::size_t kMaxOutputs = 32;
constexpr std::uint64_t kMaxTensorBytes = 512ULL * 1024ULL * 1024ULL;
constexpr std::uint64_t kMaxTotalTensorBytes = 1024ULL * 1024ULL * 1024ULL;
constexpr std::array<char, 8> kRequestMagic = {'S', 'A', 'C', 'R', 'E', 'Q', '0', '1'};
constexpr std::array<char, 8> kResponseMagic = {'S', 'A', 'C', 'R', 'E', 'S', '0', '1'};
constexpr std::uint32_t kMaxPathBytes = 32 * 1024;
constexpr std::uint32_t kMaxResponseBytes = 256 * 1024;

using Clock = std::chrono::steady_clock;

double elapsed_milliseconds(Clock::time_point started) {
  return std::chrono::duration<double, std::milli>(Clock::now() - started).count();
}

std::uint64_t peak_rss_bytes() {
  rusage usage{};
  if (getrusage(RUSAGE_SELF, &usage) != 0) return 0;
#if defined(__APPLE__)
  return static_cast<std::uint64_t>(usage.ru_maxrss);
#else
  return static_cast<std::uint64_t>(usage.ru_maxrss) * 1024ULL;
#endif
}

struct InputTensor {
  std::vector<float> values;
  std::vector<executorch::aten::SizesType> sizes;
  TensorPtr tensor;
};

template <typename T>
T read_little_endian(std::istream& input) {
  static_assert(std::is_unsigned<T>::value, "unsigned integers only");
  std::array<unsigned char, sizeof(T)> bytes{};
  input.read(reinterpret_cast<char*>(bytes.data()), bytes.size());
  if (!input) throw std::runtime_error("truncated tensor header");
  T value = 0;
  for (std::size_t index = 0; index < bytes.size(); ++index) {
    value |= static_cast<T>(bytes[index]) << (index * 8U);
  }
  return value;
}

template <typename T>
void write_little_endian(std::ostream& output, T value) {
  static_assert(std::is_unsigned<T>::value, "unsigned integers only");
  std::array<unsigned char, sizeof(T)> bytes{};
  for (std::size_t index = 0; index < bytes.size(); ++index) {
    bytes[index] = static_cast<unsigned char>((value >> (index * 8U)) & 0xffU);
  }
  output.write(reinterpret_cast<const char*>(bytes.data()), bytes.size());
}

void require_regular_file(const fs::path& path, const char* description) {
  std::error_code error;
  const auto status = fs::symlink_status(path, error);
  if (error || !fs::is_regular_file(status) || fs::is_symlink(status)) {
    throw std::runtime_error(std::string(description) + " must be a regular, non-symlink file");
  }
}

std::uint64_t checked_element_count(const std::vector<executorch::aten::SizesType>& sizes) {
  std::uint64_t count = 1;
  for (const auto size : sizes) {
    if (size < 0) throw std::runtime_error("negative tensor dimension");
    const auto dimension = static_cast<std::uint64_t>(size);
    if (dimension != 0 && count > std::numeric_limits<std::uint64_t>::max() / dimension) {
      throw std::runtime_error("tensor element count overflow");
    }
    count *= dimension;
  }
  if (count > kMaxTensorBytes / sizeof(float)) {
    throw std::runtime_error("tensor exceeds the 512 MiB limit");
  }
  return count;
}

InputTensor read_tensor(const fs::path& path) {
  require_regular_file(path, "input tensor");
  std::ifstream input(path, std::ios::binary);
  if (!input) throw std::runtime_error("cannot open input tensor");

  std::array<char, kMagic.size()> magic{};
  input.read(magic.data(), magic.size());
  if (!input || magic != kMagic) throw std::runtime_error("unsupported tensor format");
  const auto rank = read_little_endian<std::uint32_t>(input);
  if (rank > kMaxRank) throw std::runtime_error("tensor rank exceeds the supported limit");

  InputTensor result;
  result.sizes.reserve(rank);
  for (std::uint32_t index = 0; index < rank; ++index) {
    const auto dimension = read_little_endian<std::uint64_t>(input);
    if (dimension > static_cast<std::uint64_t>(std::numeric_limits<executorch::aten::SizesType>::max())) {
      throw std::runtime_error("tensor dimension exceeds the supported range");
    }
    result.sizes.push_back(static_cast<executorch::aten::SizesType>(dimension));
  }
  const auto count = checked_element_count(result.sizes);
  result.values.resize(static_cast<std::size_t>(count));
  input.read(reinterpret_cast<char*>(result.values.data()),
             static_cast<std::streamsize>(count * sizeof(float)));
  if (!input) throw std::runtime_error("truncated tensor payload");
  if (input.peek() != std::ifstream::traits_type::eof()) {
    throw std::runtime_error("unexpected bytes after tensor payload");
  }
  result.tensor = executorch::extension::from_blob(result.values.data(), result.sizes);
  return result;
}

void write_tensor(const fs::path& path, const executorch::aten::Tensor& tensor) {
  if (tensor.scalar_type() != executorch::aten::ScalarType::Float) {
    throw std::runtime_error("model output is not float32");
  }
  if (tensor.dim() > kMaxRank) throw std::runtime_error("model output rank exceeds the supported limit");
  std::vector<executorch::aten::SizesType> sizes(tensor.sizes().begin(), tensor.sizes().end());
  const auto count = checked_element_count(sizes);
  if (count != static_cast<std::uint64_t>(tensor.numel())) {
    throw std::runtime_error("model output has an inconsistent shape");
  }

  const auto temporary = fs::path(path.string() + ".tmp");
  std::ofstream output(temporary, std::ios::binary | std::ios::trunc);
  if (!output) throw std::runtime_error("cannot create output tensor");
  output.write(kMagic.data(), kMagic.size());
  write_little_endian<std::uint32_t>(output, static_cast<std::uint32_t>(sizes.size()));
  for (const auto size : sizes) write_little_endian<std::uint64_t>(output, static_cast<std::uint64_t>(size));
  const auto* data = tensor.const_data_ptr<float>();
  const auto strides = tensor.strides();
  if (strides.size() != sizes.size()) {
    throw std::runtime_error("model output has an inconsistent stride rank");
  }
  // Delegates are allowed to return channels-last or otherwise strided
  // tensors. SACTEN01 is always canonical row-major, so serialize logical
  // indices instead of copying the backing allocation blindly.
  for (std::uint64_t linear = 0; linear < count; ++linear) {
    std::uint64_t remaining = linear;
    std::uint64_t storage_offset = 0;
    for (std::size_t dimension = sizes.size(); dimension-- > 0;) {
      const auto size = static_cast<std::uint64_t>(sizes[dimension]);
      const auto coordinate = size == 0 ? 0 : remaining % size;
      remaining = size == 0 ? 0 : remaining / size;
      if (strides[dimension] < 0) throw std::runtime_error("negative output stride");
      storage_offset += coordinate * static_cast<std::uint64_t>(strides[dimension]);
    }
    const float value = data[storage_offset];
    if (!std::isfinite(value)) throw std::runtime_error("model output contains a non-finite value");
    output.write(reinterpret_cast<const char*>(&value), sizeof(value));
  }
  output.close();
  if (!output) throw std::runtime_error("failed to write output tensor");

  std::error_code error;
  fs::rename(temporary, path, error);
  if (error) {
    fs::remove(temporary);
    throw std::runtime_error("cannot publish output tensor: " + error.message());
  }
}

struct InferenceResult {
  std::size_t outputs;
  double load_milliseconds;
  double inference_milliseconds;
};

InferenceResult execute_inference(const fs::path& output_directory,
                                  const std::vector<fs::path>& input_paths,
                                  Module& module,
                                  double load_milliseconds) {
  const auto input_count = input_paths.size();
  if (input_count > kMaxInputs) throw std::runtime_error("too many model inputs");

  std::error_code error;
  if (!fs::create_directory(output_directory, error) || error) {
    throw std::runtime_error("output directory must not already exist");
  }

  std::vector<InputTensor> inputs;
  std::vector<EValue> values;
  inputs.reserve(input_count);
  values.reserve(input_count);
  std::uint64_t total_input_bytes = 0;
  for (const auto& input_path : input_paths) {
    inputs.push_back(read_tensor(input_path));
    total_input_bytes += inputs.back().values.size() * sizeof(float);
    if (total_input_bytes > kMaxTotalTensorBytes) {
      throw std::runtime_error("model inputs exceed the 1 GiB total limit");
    }
  }
  for (const auto& input : inputs) values.emplace_back(input.tensor);

  const auto inference_started = Clock::now();
  auto outputs = module.forward(values);
  const auto inference_milliseconds = elapsed_milliseconds(inference_started);
  if (!outputs.ok()) throw std::runtime_error("model inference failed");
  if (outputs->size() > kMaxOutputs) throw std::runtime_error("too many model outputs");
  for (std::size_t index = 0; index < outputs->size(); ++index) {
    if (!outputs->at(index).isTensor()) throw std::runtime_error("model output is not a tensor");
    write_tensor(output_directory / (std::to_string(index) + ".tensor"),
                 outputs->at(index).toTensor());
  }
  return {outputs->size(), load_milliseconds, inference_milliseconds};
}

std::string result_json(const InferenceResult& result) {
  return "{\"outputs\":" + std::to_string(result.outputs) +
         ",\"loadTimeMs\":" + std::to_string(result.load_milliseconds) +
         ",\"inferenceTimeMs\":" + std::to_string(result.inference_milliseconds) +
         ",\"peakRssBytes\":" + std::to_string(peak_rss_bytes()) + "}";
}

std::unique_ptr<Module> load_module(const fs::path& model_path, double& load_milliseconds) {
  require_regular_file(model_path, "model program");
  auto module = std::make_unique<Module>(model_path.string(), Module::LoadMode::MmapUseMadvise);
  const auto load_started = Clock::now();
  const auto load_error = module->load();
  load_milliseconds = elapsed_milliseconds(load_started);
  if (load_error != executorch::runtime::Error::Ok) throw std::runtime_error("cannot load model program");
  return module;
}

int infer(int argc, char** argv) {
  if (argc < 5) throw std::runtime_error("usage: infer MODEL OUTPUT_DIRECTORY INPUT...");
  const fs::path model_path(argv[2]);
  const fs::path output_directory(argv[3]);
  std::vector<fs::path> input_paths;
  for (int index = 4; index < argc; ++index) input_paths.emplace_back(argv[index]);
  double load_milliseconds = 0;
  auto module = load_module(model_path, load_milliseconds);
  const auto result = execute_inference(output_directory, input_paths, *module, load_milliseconds);
  std::cout << result_json(result) << '\n';
  return 0;
}

std::string read_framed_string(std::istream& input) {
  const auto length = read_little_endian<std::uint32_t>(input);
  if (length == 0 || length > kMaxPathBytes) throw std::runtime_error("invalid request path length");
  std::string value(length, '\0');
  input.read(value.data(), static_cast<std::streamsize>(length));
  if (!input || value.find('\0') != std::string::npos) throw std::runtime_error("invalid request path");
  return value;
}

void write_response(bool success, const std::string& payload) {
  if (payload.size() > kMaxResponseBytes) throw std::runtime_error("response exceeds protocol limit");
  std::cout.write(kResponseMagic.data(), kResponseMagic.size());
  write_little_endian<std::uint32_t>(std::cout, success ? 1U : 0U);
  write_little_endian<std::uint32_t>(std::cout, static_cast<std::uint32_t>(payload.size()));
  std::cout.write(payload.data(), static_cast<std::streamsize>(payload.size()));
  std::cout.flush();
  if (!std::cout) throw std::runtime_error("failed to write session response");
}

int serve() {
  std::string loaded_key;
  std::unique_ptr<Module> loaded_module;
  while (true) {
    std::array<char, kRequestMagic.size()> magic{};
    std::cin.read(magic.data(), magic.size());
    if (std::cin.gcount() == 0 && std::cin.eof()) return 0;
    if (!std::cin || magic != kRequestMagic) throw std::runtime_error("invalid session request magic");
    try {
      const auto path_count = read_little_endian<std::uint32_t>(std::cin);
      if (path_count < 3 || path_count > kMaxInputs + 2) {
        throw std::runtime_error("invalid session path count");
      }
      std::vector<fs::path> paths;
      paths.reserve(path_count);
      for (std::uint32_t index = 0; index < path_count; ++index) {
        paths.emplace_back(read_framed_string(std::cin));
      }
      const auto key = paths[0].string();
      double load_milliseconds = 0;
      if (!loaded_module || loaded_key != key) {
        loaded_module = load_module(paths[0], load_milliseconds);
        loaded_key = key;
      }
      std::vector<fs::path> inputs(paths.begin() + 2, paths.end());
      const auto result = execute_inference(paths[1], inputs, *loaded_module, load_milliseconds);
      write_response(true, result_json(result));
    } catch (const std::exception& error) {
      write_response(false, error.what());
    }
  }
}

int self_test(int argc, char** argv) {
  if (argc != 3) throw std::runtime_error("usage: self-test MODEL");
  const fs::path model_path(argv[2]);
  require_regular_file(model_path, "model program");
  Module module(model_path.string(), Module::LoadMode::MmapUseMadvise);
  const auto load_started = Clock::now();
  if (module.load() != executorch::runtime::Error::Ok) {
    throw std::runtime_error("cannot load model program");
  }
  const auto load_milliseconds = elapsed_milliseconds(load_started);
  auto metadata = module.method_meta("forward");
  if (!metadata.ok()) throw std::runtime_error("cannot read model metadata");
  if (metadata->num_inputs() > kMaxInputs) throw std::runtime_error("too many model inputs");

  std::vector<InputTensor> inputs;
  std::vector<EValue> values;
  inputs.reserve(metadata->num_inputs());
  values.reserve(metadata->num_inputs());
  for (std::size_t index = 0; index < metadata->num_inputs(); ++index) {
    auto tag = metadata->input_tag(index);
    if (!tag.ok() || tag.get() != executorch::runtime::Tag::Tensor) {
      throw std::runtime_error("self-test supports tensor inputs only");
    }
    auto tensor_metadata = metadata->input_tensor_meta(index);
    if (!tensor_metadata.ok() ||
        tensor_metadata->scalar_type() != executorch::aten::ScalarType::Float) {
      throw std::runtime_error("self-test supports float32 inputs only");
    }
    InputTensor input;
    input.sizes.assign(tensor_metadata->sizes().begin(), tensor_metadata->sizes().end());
    input.values.resize(static_cast<std::size_t>(checked_element_count(input.sizes)), 1.0F);
    inputs.push_back(std::move(input));
  }
  for (auto& input : inputs) {
    input.tensor = executorch::extension::from_blob(input.values.data(), input.sizes);
    values.emplace_back(input.tensor);
  }

  const auto inference_started = Clock::now();
  auto outputs = module.forward(values);
  const auto inference_milliseconds = elapsed_milliseconds(inference_started);
  if (!outputs.ok()) throw std::runtime_error("model inference failed");
  if (outputs->size() > kMaxOutputs) throw std::runtime_error("too many model outputs");
  for (const auto& output : *outputs) {
    if (!output.isTensor()) throw std::runtime_error("model output is not a tensor");
    const auto tensor = output.toTensor();
    if (tensor.scalar_type() != executorch::aten::ScalarType::Float) {
      throw std::runtime_error("model output is not float32");
    }
    const auto count = tensor.numel();
    const auto* data = tensor.const_data_ptr<float>();
    for (executorch::aten::SizesType index = 0; index < count; ++index) {
      if (!std::isfinite(data[index])) throw std::runtime_error("model output is not finite");
    }
  }
  std::cout << "{\"inputs\":" << inputs.size() << ",\"outputs\":" << outputs->size()
            << ",\"loadTimeMs\":" << load_milliseconds
            << ",\"inferenceTimeMs\":" << inference_milliseconds
            << ",\"peakRssBytes\":" << peak_rss_bytes() << "}\n";
  return 0;
}

int list_backends() {
  bool first = true;
  std::cout << '{';
  const auto append = [&first](const char* name) {
    if (!first) std::cout << ',';
    first = false;
    std::cout << '"' << name << "\":"
              << (executorch::runtime::get_backend_class(name) != nullptr ? "true" : "false");
  };
#if defined(SONARCAN_HAS_MLX)
  append("MLXBackend");
#endif
#if defined(SONARCAN_HAS_CUDA)
  append("CudaBackend");
#endif
  std::cout << "}\n";
  return 0;
}

} // namespace

int worker_main(int argc, char** argv) {
  try {
    if (argc == 2 && std::strcmp(argv[1], "--version") == 0) {
      std::cout << "sonarcan-executorch-worker 1.4.1 SACTEN01\n";
      return 0;
    }
    executorch::runtime::runtime_init();
    if (argc == 2 && std::strcmp(argv[1], "--backends") == 0) return list_backends();
    if (argc == 2 && std::strcmp(argv[1], "serve") == 0) return serve();
    if (argc >= 2 && std::strcmp(argv[1], "infer") == 0) return infer(argc, argv);
    if (argc >= 2 && std::strcmp(argv[1], "self-test") == 0) return self_test(argc, argv);
    std::cerr << "usage: sonarcan-executorch-worker (--version|--backends|serve|infer|self-test) ...\n";
    return 2;
  } catch (const std::exception& error) {
    std::cerr << "sonarcan-executorch-worker: " << error.what() << '\n';
    return 1;
  }
}

int main(int argc, char** argv) {
#if defined(__APPLE__)
  @autoreleasepool {
    return worker_main(argc, argv);
  }
#else
  return worker_main(argc, argv);
#endif
}
