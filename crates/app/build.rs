//! The Mac updater's Objective-C half is compiled on the Mac only. See native/updater.m.
fn main() {
    println!("cargo:rerun-if-changed=native/updater.m");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    cc::Build::new().file("native/updater.m").flag("-fobjc-arc").compile("atelier_updater");
    println!("cargo:rustc-link-lib=framework=AppKit");
}
