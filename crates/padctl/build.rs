//! Embed `git describe` in `padctl --version` when built from a checkout.

use std::process::Command;

fn main() {
    let pkg = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let describe = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let version = match describe {
        Some(d) => format!("{pkg} ({d})"),
        None => pkg,
    };
    println!("cargo:rustc-env=PADCTL_VERSION={version}");
    for p in [
        "../../.git/HEAD",
        "../../.git/refs/tags",
        "../../.git/index",
    ] {
        println!("cargo:rerun-if-changed={p}");
    }
}
