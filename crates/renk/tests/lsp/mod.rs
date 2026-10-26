//! Helpers for tests that drive `renk-lsp` over stdio.

use std::{
    env, fs,
    io::{BufRead, Write},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
};

use serde_json::Value;

/// The rust-analyzer on `PATH`, or `None` after saying the test is skipped.
pub fn rust_analyzer() -> Option<PathBuf> {
    let found = env::split_paths(&env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join("rust-analyzer"))
        .find(|path| {
            fs::metadata(path)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        });
    if found.is_none() {
        eprintln!("skipped: rust-analyzer is not installed");
    }
    found
}

pub fn temp_dir(name: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("renk-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn send(stdin: &mut impl Write, message: Value) {
    let body = message.to_string();
    write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    stdin.flush().unwrap();
}

pub fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse().ok()?;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}
