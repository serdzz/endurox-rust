use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Generate UBF field constants from the shared ubftab/*.fd.h tables.
///
/// Same approach as the old endurox-sys build: parse the mkfldhdr generated
/// C headers and emit `pub const <NAME>: i32 = <BFLDID32>;` for use with the
/// endurox-rs UBF API and `#[ubf(field = ...)]` derive attributes.
fn main() {
    let ubftab_dir = if let Ok(apphome) = env::var("NDRX_APPHOME") {
        PathBuf::from(apphome).join("ubftab")
    } else {
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../ubftab")
    };

    let mut rust_code = String::from("// Auto-generated UBF field constants\n");
    rust_code.push_str("// DO NOT EDIT - generated from *.fd.h files in ubftab/\n\n");

    if let Ok(entries) = fs::read_dir(&ubftab_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_fd_h = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".fd.h"));
            if !is_fd_h {
                continue;
            }

            println!("cargo:rerun-if-changed={}", path.display());
            rust_code.push_str(&format!(
                "\n// Fields from {}\n",
                path.file_name().unwrap().to_str().unwrap()
            ));
            if let Ok(content) = fs::read_to_string(&path) {
                parse_ubf_header(&content, &mut rust_code);
            }
        }
    }

    println!("cargo:rerun-if-changed=../ubftab");
    println!("cargo:rerun-if-env-changed=NDRX_APPHOME");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("ubf_fields.rs");
    let mut file = fs::File::create(&out_path).expect("Failed to create ubf_fields.rs");
    file.write_all(rust_code.as_bytes())
        .expect("Failed to write ubf_fields.rs");
}

fn parse_ubf_header(content: &str, rust_code: &mut String) {
    for line in content.lines() {
        if line.trim().starts_with("#define") && line.contains("((BFLDID32)") {
            // #define\tT_NAME_FLD\t((BFLDID32)167773162)\t/* number: 1002 type: string */
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[1];
                let value_part = parts[2];
                if let Some(start) = value_part.find("((BFLDID32)") {
                    let num_start = start + 11;
                    if let Some(end) = value_part[num_start..].find(')') {
                        let value = &value_part[num_start..num_start + end];
                        rust_code.push_str(&format!("pub const {}: i32 = {};\n", name, value));
                    }
                }
            }
        }
    }
}
