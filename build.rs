use std::process::Command;

fn main() {
    // Invalidate the built crate whenever the HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    // Also rerun if refs change (covers branch updates)
    println!("cargo:rerun-if-changed=.git/refs");

    // Try to get the short commit hash
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    // Export as an environment variable visible to the compiler
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);
}
