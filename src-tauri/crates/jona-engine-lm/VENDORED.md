# Vendored: KenLM

| | |
|---|---|
| Upstream | https://github.com/kpu/kenlm |
| Commit | `4cb443e` — 2025-03-30 (repository HEAD; upstream has been idle since) |
| Location | `kenlm-c/` — the `lm/` and `util/` subtrees only |
| License | LGPL 2.1+, query-only sources — see `kenlm-c/LICENSE` |

99 files, **byte-identical to upstream**. No local patch.

`kenlm_ffi.cc` is ours, not upstream: it is the C wrapper exposing the seven
functions `jona-engine-lm` calls.

## Checking whether we are up to date

```sh
git clone https://github.com/kpu/kenlm /tmp/kenlm
cd /tmp/kenlm
find lm util -name '*.cc' -o -name '*.hh' | while read f; do
  diff <(git show HEAD:"$f") "../path/to/kenlm-c/$f"
done
```

Any difference means either upstream moved or someone patched locally; both are
worth recording here. Dependabot does not watch vendored code.

## Build flags

`build.rs` compiles these sources directly with `cc`, without upstream's CMake.
The flags therefore live in `build.rs` and differ per toolchain:

- `_LIBCPP_ENABLE_CXX17_REMOVED_UNARY_BINARY_FUNCTION` (libc++) and
  `_HAS_AUTO_PTR_ETC` (MSVC) both bring back `std::binary_function`, which
  KenLM still uses and C++17 removed.
- `HAVE_ZLIB` / `HAVE_BZLIB` are set only off MSVC: those libraries ship with
  the macOS SDK and have no MSVC equivalent. They only enable reading
  *compressed* ARPA text — our models are uncompressed `.binary` files.
- KenLM tests `HAVE_XZLIB`, never `HAVE_LZMA`; the latter was defined here for
  a while and did nothing.
