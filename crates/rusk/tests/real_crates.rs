//! Converts every library source of pinned crates.io releases from Rust to Rusk and back, then
//! checks that each crate still compiles. Crates come from `cargo vendor`, which checks them
//! against crates.io checksums, rather than git submodules, so nothing large is checked in.
//! Downloads on first run: `cargo test -p rusk --test real_crates -- --ignored`.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use rusk::{SourceSyntax, rust_to_rusk, to_rust};

const CRATES: &[(&str, &str)] = &[
    ("anyhow", "1.0.104"),
    ("bitflags", "2.13.2"),
    ("either", "1.19.0"),
    ("hashbrown", "0.17.1"),
    ("indexmap", "2.14.2"),
    ("itertools", "0.15.0"),
    ("log", "0.4.34"),
    ("memchr", "2.8.3"),
    ("once_cell", "1.21.4"),
    ("regex-syntax", "0.8.11"),
    ("semver", "1.0.28"),
    ("serde_json", "1.0.154"),
    ("smallvec", "1.15.1"),
    ("thiserror-impl", "2.0.21"),
    ("unicode-width", "0.2.2"),
];

#[test]
#[ignore = "downloads crates from crates.io"]
fn real_crates_roundtrip_to_compilable_rust() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("real-crates");
    let vendor = vendor(&root);
    let work = root.join("work");
    let _ = fs::remove_dir_all(&work);

    let mut failures = Vec::new();
    for (name, version) in CRATES {
        let original = vendor.join(format!("{name}-{version}"));
        let converted = work.join(format!("{name}-{version}"));
        copy_crate(&original, &converted);
        for file in rust_files(&converted.join("src")) {
            let rust = fs::read_to_string(&file).unwrap();
            let shown = file.strip_prefix(&work).unwrap().display().to_string();
            match rust_to_rusk(&rust)
                .map_err(|error| format!("to Rusk: {error}"))
                .and_then(|rusk| {
                    to_rust(&rusk, SourceSyntax::Rusk).map_err(|error| format!("to Rust: {error}"))
                }) {
                Ok(roundtripped) => fs::write(&file, roundtripped).unwrap(),
                Err(error) => failures.push(format!("{shown}: {error}")),
            }
        }
        if let Err(errors) = cargo_check(&converted, &root) {
            let baseline = work.join(format!("{name}-{version}-original"));
            copy_crate(&original, &baseline);
            match cargo_check(&baseline, &root) {
                Ok(()) => failures.push(format!("{name}-{version} does not compile:\n{errors}")),
                Err(errors) => panic!("{name}-{version} does not compile unconverted:\n{errors}"),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The vendored crate sources, downloaded once into `root`.
fn vendor(root: &Path) -> PathBuf {
    let vendor = root.join("vendor");
    let fetch = root.join("fetch");
    let manifest = CRATES.iter().fold(
        "[package]\nname = \"fetch\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n\n[dependencies]\n"
            .to_string(),
        |manifest, (name, version)| manifest + &format!("{name} = \"={version}\"\n"),
    );
    // Written after vendoring, so an interrupted download is retried.
    let vendored = fetch.join("vendored.toml");
    if fs::read_to_string(&vendored).is_ok_and(|existing| existing == manifest) {
        return vendor;
    }
    fs::create_dir_all(fetch.join("src")).unwrap();
    fs::write(fetch.join("src/lib.rs"), "").unwrap();
    fs::write(fetch.join("Cargo.toml"), &manifest).unwrap();
    let status = Command::new("cargo")
        .args(["vendor", "--quiet", "--versioned-dirs", "--manifest-path"])
        .arg(fetch.join("Cargo.toml"))
        .arg(&vendor)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "cargo vendor failed");
    fs::write(vendored, manifest).unwrap();
    vendor
}

/// `cargo check` of a crate's library, or its compiler errors.
fn cargo_check(crate_dir: &Path, root: &Path) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["check", "--lib", "--quiet", "--message-format=short"])
        .current_dir(crate_dir)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .unwrap();
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let errors = stderr
        .lines()
        .filter(|line| line.contains("error"))
        .collect::<Vec<_>>();
    Err(errors[..errors.len().min(10)].join("\n"))
}

/// Copies a vendored crate as its own workspace.
fn copy_crate(from: &Path, to: &Path) {
    copy_dir(from, to);
    let manifest = to.join("Cargo.toml");
    let mut document = fs::read_to_string(&manifest)
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    document.insert("workspace", toml_edit::table());
    fs::write(manifest, document.to_string()).unwrap();
}

fn copy_dir(from: &Path, to: &Path) {
    let _ = fs::remove_dir_all(to);
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).into_iter().flatten() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}
