// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// A backend available to the local llama.cpp runtime.
nonisolated public enum LocalModelBackend: String, EnumType {
  /// Apple Metal acceleration.
  case metal = "METAL"
  /// NVIDIA CUDA acceleration.
  case cuda = "CUDA"
  /// Vulkan acceleration.
  case vulkan = "VULKAN"
  /// Portable CPU inference.
  case cpu = "CPU"
}
