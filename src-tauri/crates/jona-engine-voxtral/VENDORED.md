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

## Local modification

`voxtral_metal.m` carries one patch — everything else is byte-identical to upstream:

```c
#if defined(__MAC_OS_X_VERSION_MAX_ALLOWED) && __MAC_OS_X_VERSION_MAX_ALLOWED >= 150000
  ...
#endif
```

It guards an API that only exists in the macOS 15 SDK, so the build also works
against older SDKs.

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

## Not built off macOS

`jona-engine-voxtral` is gated behind `cfg(target_os = "macos")` in the root
`Cargo.toml`. `USE_METAL` and `USE_BLAS` are optional in the C, but
`voxtral_safetensors.c` calls `mmap`/`munmap` through `<sys/mman.h>` with no
`_WIN32` branch anywhere in the tree — porting it means writing that branch.
