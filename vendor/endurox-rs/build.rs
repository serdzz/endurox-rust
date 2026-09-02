// build.rs
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    // --- 1) Parse env flags --------------------------------------------------
    // We accept typical envs:
    //   CFLAGS   -> -I / -D (compile-time)
    //   LDFLAGS  -> -L / -l (link-time)
    let cflags = env::var("CFLAGS").unwrap_or_default();
    let ldflags = env::var("LDFLAGS").unwrap_or_default();

    let mut include_dirs: Vec<String> = Vec::new();
    let mut defines: Vec<String> = Vec::new();

    // From CFLAGS: collect -I and -D for both cc and bindgen/clang
    for flag in cflags.split_whitespace() {
        if let Some(path) = flag.strip_prefix("-I") {
            include_dirs.push(path.to_string());
        } else if let Some(def) = flag.strip_prefix("-D") {
            defines.push(def.to_string());
        } else if let Some(path) = flag.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={path}");
        } else if let Some(name) = flag.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={name}");
        }
    }

    // From LDFLAGS: pass -L/-l to rustc
    for flag in ldflags.split_whitespace() {
        if let Some(path) = flag.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={path}");
        } else if let Some(name) = flag.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={name}");
        }
    }

    // --- 2) Rebuild triggers --------------------------------------------------
    // Re-run if wrapper or any env that affects codegen changes.
    println!("cargo:rerun-if-changed=include/wrapper.h");
    println!("cargo:rerun-if-changed=tests/ubftab/test.fd");
    println!("cargo:rerun-if-env-changed=ENDUROX_MKFLDHDR");
    println!("cargo:rerun-if-env-changed=CFLAGS");
    println!("cargo:rerun-if-env-changed=LDFLAGS");
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    println!("cargo:rustc-check-cfg=cfg(endurox_pollable)");

    // Resolve Enduro/X libs and include paths via pkg-config (atmisrvinteg.pc).
    // This emits the appropriate cargo:rustc-link-lib / rustc-link-search lines.
    let library = pkg_config::Config::new()
        .probe("atmisrvinteg")
        .expect("pkg-config failed to find atmisrvinteg.pc; set PKG_CONFIG_PATH to the directory containing it");

    for path in &library.include_paths {
        let path_str = path.to_string_lossy().into_owned();
        if !include_dirs.iter().any(|d| d == &path_str) {
            include_dirs.push(path_str);
        }
    }

    generate_ubf_field_constants();

    if endurox_config_has_pollable_reply_queue(&include_dirs) {
        println!("cargo:rustc-cfg=endurox_pollable");
    }

    // --- 3) Generate bindings with bindgen -----------------------------------
    // Skip bindgen on docs.rs (no libclang). You can also gate with a feature.
    let building_docs = env::var("DOCS_RS").is_ok();
    if !building_docs {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let wrapper = manifest_dir.join("include/wrapper.h");

        let mut builder = bindgen::Builder::default()
            .header(wrapper.to_string_lossy())
            .layout_tests(false)
            // <string.h> arrives transitively through the Enduro/X headers, so
            // bindgen would redeclare libc's mem*/str* symbols. Rust code calls
            // std or libc for those and never these, and rustc's
            // suspicious_runtime_symbol_definitions lint objects to the
            // redeclaration: bindgen emits c_ulong where the compiler expects
            // usize. Same layout on a 64-bit target, a real mismatch on 32-bit.
            .blocklist_function("mem(cmp|cpy|move|set)|bcmp|strlen")
            .formatter(bindgen::Formatter::Rustfmt);

        // Forward include dirs and defines to clang so <angled> includes resolve.
        for dir in &include_dirs {
            builder = builder.clang_arg(format!("-I{dir}"));
        }
        for def in &defines {
            builder = builder.clang_arg(format!("-D{def}"));
        }

        // (Optional) curate what you pull in:
        // builder = builder
        //     .allowlist_function("my_.*")
        //     .allowlist_type("my_.*")
        //     .allowlist_var("MY_.*");

        let bindings = builder
            .generate()
            .expect("bindgen failed to generate bindings");

        let out = PathBuf::from(env::var("OUT_DIR").unwrap());
        bindings
            .write_to_file(out.join("bindings.rs"))
            .expect("Couldn't write bindings.rs");
    }
}

fn generate_ubf_field_constants() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let fd = manifest_dir.join("tests/ubftab/test.fd");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mkfldhdr = find_mkfldhdr(&manifest_dir);

    let status = Command::new(&mkfldhdr)
        .arg("-m4")
        .arg("-d")
        .arg(&out)
        .arg(&fd)
        .status()
        .unwrap_or_else(|e| panic!("failed to execute {}: {e}", mkfldhdr.display()));

    if !status.success() {
        panic!(
            "{} failed to generate Rust UBF constants from {}",
            mkfldhdr.display(),
            fd.display()
        );
    }
}

fn find_mkfldhdr(manifest_dir: &Path) -> PathBuf {
    if let Ok(path) = env::var("ENDUROX_MKFLDHDR") {
        return PathBuf::from(path);
    }

    let sibling = manifest_dir
        .parent()
        .map(|parent| parent.join("endurox/mkfldhdr/mkfldhdr"));
    if let Some(path) = sibling {
        if path.exists() {
            return path;
        }
    }

    PathBuf::from("mkfldhdr")
}

fn endurox_config_has_pollable_reply_queue(include_dirs: &[String]) -> bool {
    for dir in include_dirs {
        let cfg = PathBuf::from(dir).join("ndrx_config.h");
        let Ok(contents) = fs::read_to_string(cfg) else {
            continue;
        };
        if config_define_enabled(&contents, "EX_USE_EPOLL")
            || config_string_define_eq(&contents, "EX_POLLER_STR", "EPOLL")
            || config_define_enabled(&contents, "EX_USE_KQUEUE")
            || config_string_define_eq(&contents, "EX_POLLER_STR", "KQUEUE")
        {
            return true;
        }
    }
    false
}

fn config_define_enabled(contents: &str, name: &str) -> bool {
    contents.lines().any(|line| {
        let line = line.trim();
        line == format!("#define {name} 1") || line == format!("#define {name}")
    })
}

fn config_string_define_eq(contents: &str, name: &str, expected: &str) -> bool {
    let prefix = format!("#define {name} ");
    contents.lines().any(|line| {
        let line = line.trim();
        line.strip_prefix(&prefix)
            .and_then(|value| value.trim().trim_matches('"').split_whitespace().next())
            .is_some_and(|value| value.eq_ignore_ascii_case(expected))
    })
}
