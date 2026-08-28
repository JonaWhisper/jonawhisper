# Pins the ARM baseline for ggml. whisper-rs-sys only forwards CMAKE_* and
# WHISPER_* env vars to cmake, so CMAKE_TOOLCHAIN_FILE is the only way to set
# GGML variables directly.
#
# GGML_NATIVE=ON means "optimize the build for the current system": ggml probes
# -mcpu=native and targets the *build* machine. A release built on a newer CI
# runner would emit instructions that fault on older Apple Silicon.
#
# Keep this even though the Clang 16+ i8mm inlining error it was originally
# written for ("always_inline function 'vmmlaq_s32' requires target feature
# 'i8mm'") is fixed upstream as of whisper-rs-sys 0.15 — verified by building
# the whole workspace without this file.
#
# GGML_NATIVE must be OFF, otherwise GGML_CPU_ARM_ARCH is ignored.
set(GGML_NATIVE OFF CACHE BOOL "Disable native CPU detection" FORCE)
set(GGML_CPU_ARM_ARCH "armv8.2-a+dotprod" CACHE STRING "Force ARM arch" FORCE)
