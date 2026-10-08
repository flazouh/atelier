//! Two jobs. Every release's notes (docs/release-notes/<version>.md) are built into the app, for the changelog the version
//! in the title bar opens. And the Mac updater's Objective-C half is compiled on the Mac only. See native/updater.m.
use std::{fmt::Write as _, path::Path};

/// `0.1.10` as numbers, so that it sorts after `0.1.9`.
fn numbers(version: &str) -> Vec<u32> {
    version.split('.').map(|part| part.parse().unwrap_or(0)).collect()
}

/// Writes `releases.rs` to the build folder: `RELEASES`, every version with its notes, newest first.
fn embed_release_notes() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest folder");
    let dir = Path::new(&manifest).join("../../docs/release-notes");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut found: Vec<(String, std::path::PathBuf)> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .filter_map(|path| Some((path.file_stem()?.to_str()?.to_string(), path)))
        .collect();
    found.sort_by_key(|(version, _)| std::cmp::Reverse(numbers(version)));
    let mut code = String::from("/// Every release's notes, newest first: the version and the markdown.\npub const RELEASES: &[(&str, &str)] = &[\n");
    for (version, path) in &found {
        println!("cargo:rerun-if-changed={}", path.display());
        let path = path.canonicalize().unwrap_or_else(|_| path.clone());
        writeln!(code, "    ({version:?}, include_str!({:?})),", path.display().to_string()).expect("a string takes a write");
    }
    code.push_str("];\n");
    let out = std::env::var("OUT_DIR").expect("cargo sets the build folder");
    std::fs::write(Path::new(&out).join("releases.rs"), code).expect("the build folder is writable");
}

fn main() {
    embed_release_notes();
    println!("cargo:rerun-if-changed=native/updater.m");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    cc::Build::new().file("native/updater.m").flag("-fobjc-arc").compile("atelier_updater");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=UserNotifications");
}
