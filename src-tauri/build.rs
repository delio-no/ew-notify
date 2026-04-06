use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    tauri_build::build();

    // Copy WinDivert.dll to the output directory on Windows
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let profile = env::var("PROFILE").unwrap_or_default();

        // Navigate to the build directory to find windivert-sys output
        let build_dir = out_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.join("build"))
            .unwrap_or_default();

        // Search for the DLL in build directories
        for entry in std::fs::read_dir(&build_dir).ok().into_iter().flatten() {
            if let Ok(entry) = entry {
                let dll_path = entry.path().join("out").join("WinDivert.dll");
                if dll_path.exists() {
                    // Determine the output directory (target/<profile>/)
                    let target_dir = out_dir
                        .parent()
                        .and_then(|p| p.parent())
                        .and_then(|p| p.parent())
                        .unwrap_or_else(|| out_dir.as_path());

                    let dll_dest = target_dir.join("WinDivert.dll");
                    if let Err(e) = fs::copy(&dll_path, &dll_dest) {
                        println!("cargo:warning=Failed to copy WinDivert.dll: {}", e);
                    } else {
                        println!("cargo:warning=Copied WinDivert.dll to output directory");
                    }
                    break;
                }
            }
        }
    }
}
