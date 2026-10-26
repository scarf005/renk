#![cfg(unix)]

mod lsp;

use std::{
    env, fs,
    io::BufReader,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use lsp::{read_message, rust_analyzer, send, temp_dir};
use serde_json::{Value, json};

const SCRIPT: &str = "#!/usr/bin/env -S renk script\n---\n[package]\nedition = \"2024\"\n---\nlet name = String.from(\"a\")\nprintln!(\"{name}\")\n";

/// Opens a script with the first text, sends the rest as edits right away, and hovers
/// `String.from` in `SCRIPT`, which the last text must be. The first rust-analyzer starts two
/// seconds late, so the edits reach renk-lsp while it waits for it, and any second one would be
/// ready first. `None` without rust-analyzer.
fn hover_after_edits(name: &str, texts: &[&str]) -> Option<Value> {
    let rust_analyzer = rust_analyzer()?;
    let dir = temp_dir(name);
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let slow = bin.join("rust-analyzer");
    fs::write(
        &slow,
        "#!/bin/sh\nif mkdir \"$RENK_TEST_STARTED\" 2>/dev/null; then sleep 2; fi\nexec \"$RENK_TEST_RUST_ANALYZER\" \"$@\"\n",
    )
    .unwrap();
    fs::set_permissions(&slow, fs::Permissions::from_mode(0o755)).unwrap();
    let path = env::join_paths(
        std::iter::once(bin).chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
    )
    .unwrap();
    let script = dir.join("tool.rk");
    let text = texts.last().copied().unwrap();
    fs::write(&script, text).unwrap();
    let uri = format!("file://{}", script.display());

    let mut server = Command::new(env!("CARGO_BIN_EXE_renk-lsp"))
        .env("PATH", &path)
        .env("RENK_TEST_STARTED", dir.join("started"))
        .env("RENK_TEST_RUST_ANALYZER", &rust_analyzer)
        .env("XDG_CACHE_HOME", dir.join("cache"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = server.stdin.take().unwrap();
    let (messages, received) = mpsc::channel();
    let mut stdout = BufReader::new(server.stdout.take().unwrap());
    thread::spawn(move || {
        while let Some(message) = read_message(&mut stdout) {
            if messages.send(message).is_err() {
                break;
            }
        }
    });
    let response = |id: u64| loop {
        let message = received.recv_timeout(Duration::from_secs(30)).unwrap();
        if message["id"] == id && message.get("method").is_none() {
            return message["result"].clone();
        }
    };

    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "processId": null,
            "rootUri": format!("file://{}", dir.display()),
            "capabilities": {},
        }}),
    );
    response(1);
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
            "textDocument": {"uri": uri, "languageId": "renk", "version": 1, "text": texts[0]},
        }}),
    );
    for (version, text) in texts.iter().enumerate().skip(1) {
        send(
            &mut stdin,
            json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
                "textDocument": {"uri": uri, "version": version + 1},
                "contentChanges": [{"text": text}],
            }}),
        );
    }

    // rust-analyzer answers hovers once it has loaded the script's generated cargo-script file.
    let column = "let name = String.from".len() - 2;
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut id = 2;
    let hover = loop {
        send(
            &mut stdin,
            json!({"jsonrpc": "2.0", "id": id, "method": "textDocument/hover", "params": {
                "textDocument": {"uri": uri},
                "position": {"line": 5, "character": column},
            }}),
        );
        let hover = response(id);
        if hover["contents"].to_string().contains("fn from") || Instant::now() > deadline {
            break hover;
        }
        id += 1;
        thread::sleep(Duration::from_millis(500));
    };
    let _ = server.kill();
    let _ = server.wait();
    let _ = fs::remove_dir_all(&dir);
    Some(hover)
}

#[test]
fn script_hover_comes_from_rust_analyzer_over_the_scripts_generated_rust() {
    let Some(hover) = hover_after_edits("lsp-script", &[SCRIPT]) else {
        return;
    };
    assert!(hover["contents"].to_string().contains("fn from"), "{hover}");
    assert_eq!(hover["range"]["start"]["line"], 5, "{hover}");
}

#[test]
fn script_edits_sent_while_rust_analyzer_starts_reach_it_in_order() {
    let draft = SCRIPT.replace("String.from(\"a\")", "1");
    let Some(hover) = hover_after_edits("lsp-script-edit", &[&draft, SCRIPT]) else {
        return;
    };
    assert!(hover["contents"].to_string().contains("fn from"), "{hover}");
}

/// The live rust-analyzer processes renk-lsp has started.
#[cfg(target_os = "linux")]
fn rust_analyzer_children(pid: u32) -> usize {
    let children = fs::read_dir(format!("/proc/{pid}/task"))
        .unwrap()
        .flat_map(|task| fs::read_to_string(task.unwrap().path().join("children")))
        .collect::<String>();
    children
        .split_whitespace()
        .filter(|child| {
            let stat = fs::read_to_string(format!("/proc/{child}/stat")).unwrap_or_default();
            stat.contains("(rust-analyzer)") && !stat.contains(") Z ")
        })
        .count()
}

#[cfg(target_os = "linux")]
#[test]
fn a_script_that_becomes_a_module_leaves_its_own_rust_analyzer() {
    if rust_analyzer().is_none() {
        return;
    }
    let dir = temp_dir("lsp-script-to-module");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"tool\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let module = "fn main():\n    println!(\"hi\")\n";
    let script = format!("#!/usr/bin/env -S renk script\n{module}");
    let path = dir.join("src/main.rk");
    fs::write(&path, module).unwrap();
    let uri = format!("file://{}", path.display());

    let mut server = Command::new(env!("CARGO_BIN_EXE_renk-lsp"))
        .env("XDG_CACHE_HOME", dir.join("cache"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = server.stdin.take().unwrap();
    let mut stdout = BufReader::new(server.stdout.take().unwrap());
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "processId": null,
            "rootUri": format!("file://{}", dir.display()),
            "capabilities": {},
        }}),
    );
    while read_message(&mut stdout).unwrap()["id"] != 1 {}
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    for (version, text) in [&script, module].into_iter().enumerate() {
        let message = if version == 0 {
            json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
                "textDocument": {"uri": uri, "languageId": "renk", "version": 1, "text": text},
            }})
        } else {
            json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
                "textDocument": {"uri": uri, "version": version + 1},
                "contentChanges": [{"text": text}],
            }})
        };
        send(&mut stdin, message);
    }

    // renk-lsp publishes a document's diagnostics after syncing it with rust-analyzer.
    let mut synced = 0;
    while synced < 2 {
        if read_message(&mut stdout).unwrap()["method"] == "textDocument/publishDiagnostics" {
            synced += 1;
        }
    }
    thread::spawn(move || while read_message(&mut stdout).is_some() {});

    // The workspace rust-analyzer now serves the module, and the script's one has exited.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut running = rust_analyzer_children(server.id());
    while running != 1 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(200));
        running = rust_analyzer_children(server.id());
    }
    let _ = server.kill();
    let _ = server.wait();
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(running, 1);
}
