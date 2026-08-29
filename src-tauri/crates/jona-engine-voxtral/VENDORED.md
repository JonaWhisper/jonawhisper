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

Three files differ from upstream; the other twelve are byte-identical.

### `voxtral_safetensors.c` — file mapping behind a portability shim

`safetensors_open()` called `open`/`fstat`/`mmap` directly, with no `_WIN32`
branch. The calls now sit in `vox_map_readonly()` / `vox_unmap()`, which keep
the exact same POSIX path and add a Windows one built on `CreateFileMapping` +
`MapViewOfFile`. Windows keeps the view valid once the mapping handle is
closed, so unmapping needs only the pointer.

### `voxtral.c` — `gettimeofday`

`windows.h` has no `struct timeval` (it lives in `winsock2.h`, which this tree
does not pull in). Under `_WIN32` the file defines the two fields it uses and
fills them from `GetSystemTimeAsFileTime`. Timing only — it feeds the
`encoder_ms` counters.

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

Only `voxtral_metal.m` should differ, by the block above. Dependabot does not
watch vendored code — this file is the only record of where it came from.

## Building off macOS

`USE_METAL`, `USE_BLAS` and `ACCELERATE_NEW_LAPACK` are set only on macOS, and
`voxtral_metal.m` is compiled there only. Everywhere else the same C builds
without them, on CPU — slower, and the model is 8.9 GB, so whether that is
worth shipping is a product question rather than a build one.
