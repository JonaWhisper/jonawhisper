# Vendored: voxtral.c

| | |
|---|---|
| Upstream | https://github.com/antirez/voxtral.c |
| Commit | `134d366` — 2026-02-15 (repository HEAD at the time of writing) |
| Location | `voxtral-c/` |
| License | see `voxtral-c/` upstream headers |

15 files, all from upstream. Four upstream files are deliberately **not** copied
because they belong to the standalone CLI, not to the library we link against:
`main.c`, `inspect_weights.c`, `voxtral_mic.h`, `voxtral_mic_macos.c`.

## Local modifications

Four files differ from upstream; the other eleven are byte-identical.

### `voxtral_safetensors.c` — file mapping behind a portability shim

`safetensors_open()` called `open`/`fstat`/`mmap` directly, with no `_WIN32`
branch. The calls now sit in `vox_map_readonly()` / `vox_unmap()`, which keep
the exact same POSIX path and add a Windows one built on `CreateFileMapping` +
`MapViewOfFile`. Windows keeps the view valid once the mapping handle is
closed, so unmapping needs only the pointer.

### `voxtral.c` — `gettimeofday`

There is no `gettimeofday` on Windows, so under `_WIN32` the file supplies one
backed by `GetSystemTimeAsFileTime`. `struct timeval` itself comes from
`winsock.h`, which `windows.h` pulls in; we define it only behind
`_TIMEVAL_DEFINED`, the guard that header uses. Timing only — it feeds the
`encoder_ms` counters.

### `voxtral_kernels.c` — the fourth `cblas_sgemm`

`vox_conv1d()` called `cblas_sgemm` directly while the three other call sites
(`vox_matmul`, `vox_matmul_t`, `vox_linear`) each guard theirs with `#ifdef
USE_BLAS` and a scalar `#else`. Upstream never hit it because `USE_BLAS` is
always on for its macOS target. The call is now `vox_matmul()`, whose BLAS
branch is that same `sgemm` argument for argument — so macOS is unchanged and
non-BLAS targets get the scalar path. Worth sending upstream.

### `voxtral_metal.m` — SDK guard

```c
#if defined(__MAC_OS_X_VERSION_MAX_ALLOWED) && __MAC_OS_X_VERSION_MAX_ALLOWED >= 150000
  ...
#endif
```

It guards an API that only exists in the macOS 15 SDK, so the build also works
against older SDKs. This file is compiled on macOS only.

## Checking whether we are up to date

```sh
git clone https://github.com/antirez/voxtral.c /tmp/vox
cd /tmp/vox
for f in ../path/to/voxtral-c/*.{c,h,m}; do
  diff <(git show HEAD:"$(basename "$f")") "$f"
done
```

The four files above should be the only ones that differ. Dependabot does not
watch vendored code — this file is the only record of where it came from.

## Building off macOS

`USE_METAL`, `USE_BLAS` and `ACCELERATE_NEW_LAPACK` are set only on macOS, and
`voxtral_metal.m` is compiled there only. Everywhere else the same C builds
without them, on CPU — slower, and the model is 8.9 GB, so whether that is
worth shipping is a product question rather than a build one.

To exercise the non-Apple preprocessor paths without a Windows box — it catches
an unguarded BLAS or Metal symbol, which is how the `vox_conv1d` one surfaced:

```sh
for f in voxtral-c/*.c; do clang -fsyntax-only -I voxtral-c "$f" || break; done
```
