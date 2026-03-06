fn main() {
    #[cfg(target_os = "macos")]
    build_voxtral();
}

#[cfg(target_os = "macos")]
fn build_voxtral() {
    let voxtral_dir = "voxtral-c";
    let out_dir = std::env::var("OUT_DIR").unwrap();

    // 1. Generate shader header (equivalent to xxd -i voxtral_shaders.metal)
    let shader_src = std::fs::read(format!("{}/voxtral_shaders.metal", voxtral_dir))
        .expect("Failed to read voxtral_shaders.metal");
    let header_path = format!("{}/voxtral_shaders_source.h", out_dir);
    let mut header = String::from(
        "// Auto-generated from voxtral_shaders.metal\n\
         static const unsigned char voxtral_shaders_metal[] = {\n"
    );
    for (i, byte) in shader_src.iter().enumerate() {
        if i % 16 == 0 { header.push_str("    "); }
        header.push_str(&format!("0x{:02x},", byte));
        if i % 16 == 15 { header.push('\n'); }
    }
    header.push_str(&format!(
        "\n}};\nstatic const unsigned int voxtral_shaders_metal_len = {};\n",
        shader_src.len()
    ));
    std::fs::write(&header_path, &header).expect("Failed to write shader header");

    // 2. Compile C sources
    let c_files = [
        "voxtral.c",
        "voxtral_kernels.c",
        "voxtral_audio.c",
        "voxtral_encoder.c",
        "voxtral_decoder.c",
        "voxtral_tokenizer.c",
        "voxtral_safetensors.c",
    ];

    let mut build = cc::Build::new();
    build
        .warnings(false)  // vendored code, not our policy
        .opt_level_str("3")
        .flag("-ffast-math")
        .define("USE_METAL", None)
        .define("USE_BLAS", None)
        .define("ACCELERATE_NEW_LAPACK", None)
        .include(voxtral_dir);
    for f in &c_files {
        build.file(format!("{}/{}", voxtral_dir, f));
    }
    build.compile("voxtral_c");

    // 3. Compile Metal ObjC (.m) separately with ARC
    cc::Build::new()
        .warnings(false)
        .opt_level_str("3")
        .flag("-fobjc-arc")
        .flag("-ffast-math")
        .define("USE_METAL", None)
        .include(voxtral_dir)
        .include(&out_dir)  // for voxtral_shaders_source.h
        .file(format!("{}/voxtral_metal.m", voxtral_dir))
        .compile("voxtral_metal");

    // 4. Link Metal frameworks
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=MetalPerformanceShaders");
    println!("cargo:rustc-link-lib=framework=MetalPerformanceShadersGraph");
    println!("cargo:rustc-link-lib=framework=Accelerate");

    println!("cargo:rerun-if-changed={}", voxtral_dir);
}
