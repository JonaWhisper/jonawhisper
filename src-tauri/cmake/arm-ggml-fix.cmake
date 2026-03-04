# Force ARM architecture for ggml to avoid Clang 16+ i8mm inlining error.
# whisper-rs-sys only passes CMAKE_* and WHISPER_* env vars to cmake,
# so we use CMAKE_TOOLCHAIN_FILE to set GGML variables directly.
#
# Without this, GGML_NATIVE=ON auto-detects CPU features and enables i8mm
# code paths that trigger: "always_inline function 'vmmlaq_s32' requires
# target feature 'i8mm'" on Clang 16+ (Xcode 16+).
#
# GGML_NATIVE must be OFF — otherwise GGML_CPU_ARM_ARCH is ignored and
# the native detection path (-mcpu=native) is used instead.
set(GGML_NATIVE OFF CACHE BOOL "Disable native CPU detection" FORCE)
set(GGML_CPU_ARM_ARCH "armv8.2-a+dotprod" CACHE STRING "Force ARM arch" FORCE)
