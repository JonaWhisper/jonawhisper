fn main() {
    let kenlm_dir = "kenlm-c";

    // Collect all C++ source files (lm/ + util/ + util/double-conversion/ + FFI wrapper)
    let mut sources: Vec<String> = Vec::new();

    // lm/*.cc
    for entry in std::fs::read_dir(format!("{kenlm_dir}/lm")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "cc") {
            sources.push(path.to_string_lossy().into_owned());
        }
    }

    // util/*.cc
    for entry in std::fs::read_dir(format!("{kenlm_dir}/util")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "cc") {
            sources.push(path.to_string_lossy().into_owned());
        }
    }

    // util/double-conversion/*.cc
    for entry in std::fs::read_dir(format!("{kenlm_dir}/util/double-conversion")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "cc") {
            sources.push(path.to_string_lossy().into_owned());
        }
    }

    // FFI wrapper
    sources.push(format!("{kenlm_dir}/kenlm_ffi.cc"));

    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .warnings(false) // vendored code
        .opt_level_str("3")
        .std("c++17")
        .define("KENLM_MAX_ORDER", "6")
        // KenLM uses std::binary_function, removed in C++17 libc++
        .define("_LIBCPP_ENABLE_CXX17_REMOVED_UNARY_BINARY_FUNCTION", None)
        // Include root so headers like "lm/model.hh" and "util/file.hh" resolve
        .include(kenlm_dir);

    if msvc {
        build.flag("/fp:fast");
    } else {
        build.flag("-ffast-math");
        // zlib/bzip2/lzma ship with the macOS SDK; MSVC has none of them, and
        // KenLM only needs them to read compressed ARPA text, not .binary models.
        build
            .define("HAVE_ZLIB", None)
            .define("HAVE_BZLIB", None)
            .define("HAVE_LZMA", None);
    }

    for src in &sources {
        build.file(src);
    }

    build.compile("kenlm");

    if !msvc {
        // Compression libraries (available in the macOS SDK)
        println!("cargo:rustc-link-lib=z");
        println!("cargo:rustc-link-lib=bz2");
        println!("cargo:rustc-link-lib=lzma");
        // C++ standard library; MSVC links its own automatically
        println!("cargo:rustc-link-lib=c++");
    }

    println!("cargo:rerun-if-changed={kenlm_dir}");
}
