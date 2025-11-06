use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

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

    // Generate UBF field constants from test.fd.h
    generate_ubf_constants();
}

fn generate_ubf_constants() {
    let ubftab_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../ubftab");

    if !ubftab_dir.exists() {
        println!("cargo:warning=ubftab directory not found, skipping UBF constants generation");
        return;
    }

    // Parse constants from all *.fd.h files
    let mut rust_code = String::from("// Auto-generated UBF field constants\n");
    rust_code.push_str("// DO NOT EDIT - generated from *.fd.h files in ubftab/\n\n");

    let mut found_files = false;

    // Read all .fd.h files in ubftab directory
    if let Ok(entries) = fs::read_dir(&ubftab_dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Process only *.fd.h files
            if let Some(ext) = path.extension() {
                if ext == "h"
                    && path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .ends_with(".fd.h")
                {
                    found_files = true;

                    let filename = path.file_name().unwrap().to_str().unwrap();
                    println!("cargo:rerun-if-changed=../ubftab/{}", filename);

                    rust_code.push_str(&format!("\n// Fields from {}\n", filename));

                    if let Ok(content) = fs::read_to_string(&path) {
                        parse_ubf_header(&content, &mut rust_code);
                    }
                }
            }
        }
    }

    if !found_files {
        println!(
            "cargo:warning=No *.fd.h files found in ubftab/, skipping UBF constants generation"
        );
        return;
    }

    // Write generated Rust code
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("ubf_fields.rs");
    let mut file = fs::File::create(&out_path).expect("Failed to create ubf_fields.rs");
    file.write_all(rust_code.as_bytes())
        .expect("Failed to write ubf_fields.rs");

    // Watch for changes in ubftab directory
    println!("cargo:rerun-if-changed=../ubftab");
}

fn parse_ubf_header(content: &str, rust_code: &mut String) {
    for line in content.lines() {
        if line.trim().starts_with("#define") && line.contains("((BFLDID32)") {
            // Parse line like:
            // #define	T_NAME_FLD	((BFLDID32)167773162)	/* number: 1002	 type: string */
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[1];
                let value_part = parts[2];

                // Extract number from ((BFLDID32)167773162)
                if let Some(start) = value_part.find("((BFLDID32)") {
                    let num_start = start + 11; // length of "((BFLDID32)"
                    if let Some(end) = value_part[num_start..].find(')') {
                        let value = &value_part[num_start..num_start + end];

                        // Extract comment for documentation
                        if let Some(comment_start) = line.find("/*") {
                            if let Some(comment_end) = line.find("*/") {
                                let _comment = line[comment_start + 2..comment_end].trim();
                                //rust_code.push_str(&format!("/// {}\n", comment));
                            }
                        }

                        rust_code.push_str(&format!("pub const {}: i32 = {};\n\n", name, value));
                    }
                }
            }
        }
    }
}
