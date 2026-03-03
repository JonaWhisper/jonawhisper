# Force ARM architecture for ggml to avoid Clang 16+ i8mm inlining error.
# whisper-rs-sys only passes CMAKE_* and WHISPER_* env vars to cmake,
# so we use CMAKE_TOOLCHAIN_FILE to set GGML variables directly.
#
# Without this, GGML_NATIVE=ON auto-detects CPU features and enables i8mm
# code paths that trigger: "always_inline function 'vmmlaq_s32' requires
# target feature 'i8mm'" on Clang 16+ (Xcode 16+).
set(GGML_CPU_ARM_ARCH "armv8.2-a+dotprod" CACHE STRING "Force ARM arch" FORCE)
