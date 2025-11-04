fn main() {
    // Link Enduro/X server library for server binaries
    println!("cargo:rustc-link-lib=atmisrv");
    println!("cargo:rerun-if-changed=build.rs");
}
