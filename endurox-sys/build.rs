fn main() {
    // Add Enduro/X library paths
    let ndrx_home = std::env::var("NDRX_HOME").unwrap_or_else(|_| "/opt/endurox".to_string());
    
    println!("cargo:rustc-link-search=native={}/lib", ndrx_home);
    
    // Common libraries for both server and client
    println!("cargo:rustc-link-lib=atmi");
    println!("cargo:rustc-link-lib=ubf");
    println!("cargo:rustc-link-lib=netproto");
    println!("cargo:rustc-link-lib=nstd");
    println!("cargo:rustc-link-lib=pthread");
    
    // rt library only exists on Linux
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=rt");
    
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=dl");
    
    println!("cargo:rustc-link-lib=m");
    
    println!("cargo:rerun-if-env-changed=NDRX_HOME");
    println!("cargo:rerun-if-changed=build.rs");
}
