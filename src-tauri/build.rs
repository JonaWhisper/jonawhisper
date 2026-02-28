fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=AVFoundation");

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
