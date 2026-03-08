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

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .warnings(false) // vendored code
        .opt_level_str("3")
        .flag("-std=c++17")
        .flag("-ffast-math")
        .define("KENLM_MAX_ORDER", "6")
        .define("HAVE_ZLIB", None)
        .define("HAVE_BZLIB", None)
        .define("HAVE_LZMA", None)
        // KenLM uses std::binary_function, removed in C++17 libc++
        .define("_LIBCPP_ENABLE_CXX17_REMOVED_UNARY_BINARY_FUNCTION", None)
        // Include root so headers like "lm/model.hh" and "util/file.hh" resolve
        .include(kenlm_dir);

    for src in &sources {
        build.file(src);
    }

    build.compile("kenlm");

    // Link compression libraries (available in macOS SDK)
    println!("cargo:rustc-link-lib=z");
    println!("cargo:rustc-link-lib=bz2");
    println!("cargo:rustc-link-lib=lzma");
    // C++ standard library
    println!("cargo:rustc-link-lib=c++");

    println!("cargo:rerun-if-changed={kenlm_dir}");
}
