fn main() {
    embuild::espidf::sysenv::output();
    git_main();
}

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    Some(s.trim().to_string())
}

fn git_main() {
    // Rerun when HEAD or the optional path changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
    if let Ok(path) = std::env::var("COMMIT_COUNT_PATH") {
        println!("cargo:rerun-if-changed={}", path);
    }

    let short_len: usize = std::env::var("SHORT_HASH_LEN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7);
    let full = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let short = full.chars().take(short_len).collect::<String>();
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", short);

    let repo_count = git(&["rev-list", "--count", "HEAD"]).unwrap_or_else(|| "0".into());
    println!("cargo:rustc-env=GIT_COMMIT_COUNT={}", repo_count);
}
