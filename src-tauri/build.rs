fn main() {
    // Auto-generate `extern crate` for all jona-engine-* and jona-provider-*
    // dependencies. This forces the linker to include them so their
    // `inventory::submit!` registrations are present at runtime.
    generate_inventory_links();

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=ServiceManagement");

        // whisper-rs Metal code uses @available() which emits __isPlatformVersionAtLeast.
        // This symbol lives in the clang compiler runtime — link it explicitly.
        let output = std::process::Command::new("xcrun")
            .args(["--show-sdk-path"])
            .output()
            .expect("xcrun failed");
        let sdk = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // Derive the clang runtime path from the SDK path
        // SDK: .../Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk
        // Runtime: .../Toolchains/XcodeDefault.xctoolchain/usr/lib/clang/<ver>/lib/darwin/
        let toolchain_base = sdk
            .split("/Platforms/")
            .next()
            .unwrap_or(&sdk);
        let clang_dir = format!(
            "{}/Toolchains/XcodeDefault.xctoolchain/usr/lib/clang",
            toolchain_base
        );
        if let Ok(entries) = std::fs::read_dir(&clang_dir) {
            for entry in entries.flatten() {
                let rt_path = entry.path().join("lib/darwin/libclang_rt.osx.a");
                if rt_path.exists() {
                    println!(
                        "cargo:rustc-link-search=native={}",
                        rt_path.parent().unwrap().display()
                    );
                    println!("cargo:rustc-link-lib=static=clang_rt.osx");
                    break;
                }
            }
        }
    }

    tauri_build::build()
}

/// Scan Cargo.toml for `jona-engine-*` and `jona-provider-*` dependencies
/// and emit a file with `extern crate` declarations. This makes registration
/// truly plug-and-play: add a dep in Cargo.toml → done.
fn generate_inventory_links() {
    let cargo_toml = std::fs::read_to_string("Cargo.toml")
        .expect("Failed to read Cargo.toml");

    let mut extern_lines = Vec::new();
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        // Match lines like: jona-engine-whisper = { ... } or jona-provider-openai = { ... }
        if let Some(name) = trimmed.strip_suffix(|_: char| true).and(None).or_else(|| {
            let dep = trimmed.split('=').next()?.trim();
            if dep.starts_with("jona-engine-") || dep.starts_with("jona-provider-") || dep.starts_with("jona-detector-") {
                Some(dep)
            } else {
                None
            }
        }) {
            let crate_name = name.replace('-', "_");
            extern_lines.push(format!("extern crate {};", crate_name));
        }
    }

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let path = std::path::Path::new(&out_dir).join("inventory_links.rs");
    std::fs::write(&path, extern_lines.join("\n")).expect("Failed to write inventory_links.rs");

    println!("cargo:rerun-if-changed=Cargo.toml");
}
