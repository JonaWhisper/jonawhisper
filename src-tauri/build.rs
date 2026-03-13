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

/// Scan Cargo.toml for `jona-engine-*`, `jona-provider-*` and `jona-detector-*`
/// dependencies and emit a file with `extern crate` declarations. This makes
/// registration truly plug-and-play: add a dep in Cargo.toml → done.
///
/// Dependencies under `[target.'cfg(...)'.dependencies]` sections get wrapped
/// with the corresponding `#[cfg(...)]` attribute.
fn generate_inventory_links() {
    let cargo_toml = std::fs::read_to_string("Cargo.toml")
        .expect("Failed to read Cargo.toml");

    let mut extern_lines = Vec::new();
    // Track current target cfg from section headers like [target.'cfg(target_os = "macos")'.dependencies]
    let mut current_cfg: Option<String> = None;

    for line in cargo_toml.lines() {
        let trimmed = line.trim();

        // Detect TOML section headers
        if trimmed.starts_with('[') {
            if let Some(cfg) = extract_target_cfg(trimmed) {
                current_cfg = Some(cfg);
            } else {
                current_cfg = None;
            }
            continue;
        }

        // Match dependency lines like: jona-engine-whisper = { ... }
        // Take the part before '=' or whitespace, then check for our prefixes.
        let dep = trimmed.split('=').next().unwrap_or("").trim();
        if dep.starts_with("jona-engine-") || dep.starts_with("jona-provider-") || dep.starts_with("jona-detector-") {
            let name = dep;
            let crate_name = name.replace('-', "_");
            if let Some(ref cfg) = current_cfg {
                extern_lines.push(format!("#[cfg({cfg})]\nextern crate {crate_name};"));
            } else {
                extern_lines.push(format!("extern crate {crate_name};"));
            }
        }
    }

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let path = std::path::Path::new(&out_dir).join("inventory_links.rs");
    std::fs::write(&path, extern_lines.join("\n")).expect("Failed to write inventory_links.rs");

    println!("cargo:rerun-if-changed=Cargo.toml");
}

/// Extract the cfg predicate from a target section header.
/// e.g. `[target.'cfg(target_os = "macos")'.dependencies]` → `target_os = "macos"`
/// The returned string is the inner predicate, ready to use in `#[cfg(...)]`.
fn extract_target_cfg(header: &str) -> Option<String> {
    // Match: [target.'cfg(...)'.dependencies]
    let inner = header.trim_start_matches('[').trim_end_matches(']').trim();
    if !inner.starts_with("target.") || !inner.ends_with(".dependencies") {
        return None;
    }
    // Extract the predicate inside cfg(...)
    let cfg_start = inner.find("cfg(")? + 4; // skip "cfg("
    let cfg_end = inner[cfg_start..].rfind(')')? + cfg_start;
    let predicate = &inner[cfg_start..cfg_end];
    Some(predicate.to_string())
}
