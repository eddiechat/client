use std::process::Command;

fn main() {
    // Get git tag version at build time - only tags with "v" prefix
    let git_version = Command::new("git")
        .args(["describe", "--tags", "--match", "v*", "--abbrev=0"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|v| v.trim().trim_start_matches('v').to_string())
        .unwrap_or_else(|| "0.1.0".to_string());

    println!("cargo:rustc-env=GIT_VERSION={}", git_version);

    tauri_build::build()
}
