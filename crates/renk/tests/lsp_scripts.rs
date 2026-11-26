#![cfg(unix)]

mod lsp;

use std::{
    env, fs,
    io::BufReader,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use lsp::rust_analyzer_children;
use lsp::{read_message, rust_analyzer, send, temp_dir};
use serde_json::{Value, json};

const SCRIPT: &str = "#!/usr/bin/env -S renk script\n---\n[package]\nedition = \"2024\"\n---\nlet name = String.from(\"a\")\nprintln!(\"{name}\")\n";

/// renk-lsp over `dir` after `initialize`, its stdin, and the messages it sends. `rust_analyzer` is
/// the shell script it runs as rust-analyzer, which gets `dir` as `$RENK_TEST_DIR` and the real
/// rust-analyzer, if any, as `$RENK_TEST_RUST_ANALYZER`.
fn start(dir: &Path, rust_analyzer: &str) -> (Child, ChildStdin, Receiver<Value>) {
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("rust-analyzer"), rust_analyzer).unwrap();
    fs::set_permissions(bin.join("rust-analyzer"), fs::Permissions::from_mode(0o755)).unwrap();
    let real = lsp::rust_analyzer().unwrap_or_default();
    let path = env::join_paths(
        std::iter::once(bin).chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
    )
    .unwrap();

    let mut server = Command::new(env!("CARGO_BIN_EXE_renk-lsp"))
        .env("PATH", &path)
        .env("RENK_TEST_DIR", dir)
        .env("RENK_TEST_RUST_ANALYZER", real)
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
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "processId": null,
            "rootUri": format!("file://{}", dir.display()),
            "capabilities": {},
        }}),
    );
    response(&received, 1);
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    (server, stdin, received)
}

/// The result of the response to request `id`, within 30 seconds.
fn response(received: &Receiver<Value>, id: u64) -> Value {
    loop {
        let message = received.recv_timeout(Duration::from_secs(30)).unwrap();
        if message["id"] == id && message.get("method").is_none() {
            return message["result"].clone();
        }
    }
}

/// Opens a script with the first text, sends the rest as edits right away, and hovers
/// `String.from` in `SCRIPT`, which the last text must be. The first rust-analyzer starts two
/// seconds late, so the edits reach renk-lsp while it waits for it, and any second one would be
/// ready first. With `restart`, kills rust-analyzer once it answers the hover, and edits and
/// hovers again until a new one answers. `None` without rust-analyzer.
fn hover_after_edits(name: &str, texts: &[&str], restart: bool) -> Option<Value> {
    rust_analyzer()?;
    let dir = temp_dir(name);
    let script = dir.join("tool.rk");
    let text = texts.last().copied().unwrap();
    fs::write(&script, text).unwrap();
    let uri = format!("file://{}", script.display());
    let slow = "#!/bin/sh\nif mkdir \"$RENK_TEST_DIR/started\" 2>/dev/null; then sleep 2; fi\nexec \"$RENK_TEST_RUST_ANALYZER\" \"$@\"\n";
    let (mut server, mut stdin, received) = start(&dir, slow);
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
    let mut deadline = Instant::now() + Duration::from_secs(120);
    let mut id = 2;
    let mut killed: Option<u32> = None;
    let hover = loop {
        // renk-lsp starts rust-analyzer again for a change, so change until a new one runs.
        // Further changes would cancel its work on a hover that renk-lsp stopped waiting for.
        #[cfg(target_os = "linux")]
        let restarting = killed.is_some_and(|killed| {
            !rust_analyzer_children(server.id())
                .iter()
                .any(|&child| child != killed)
        });
        #[cfg(not(target_os = "linux"))]
        let restarting = false;
        if restarting {
            // Each id is above every version sent before, so it serves as the next version.
            send(
                &mut stdin,
                json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
                    "textDocument": {"uri": uri, "version": id},
                    "contentChanges": [{"text": text}],
                }}),
            );
        }
        send(
            &mut stdin,
            json!({"jsonrpc": "2.0", "id": id, "method": "textDocument/hover", "params": {
                "textDocument": {"uri": uri},
                "position": {"line": 5, "character": column},
            }}),
        );
        let hover = response(&received, id);
        let found = hover["contents"].to_string().contains("fn from");
        if found && restart && killed.is_none() {
            #[cfg(target_os = "linux")]
            {
                killed = Some(lsp::kill_rust_analyzer(server.id()));
            }
            deadline = Instant::now() + Duration::from_secs(120);
        } else if found || Instant::now() > deadline {
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
    let Some(hover) = hover_after_edits("lsp-script", &[SCRIPT], false) else {
        return;
    };
    assert!(hover["contents"].to_string().contains("fn from"), "{hover}");
    assert_eq!(hover["range"]["start"]["line"], 5, "{hover}");
}

#[test]
fn script_edits_sent_while_rust_analyzer_starts_reach_it_in_order() {
    let draft = SCRIPT.replace("String.from(\"a\")", "1");
    let Some(hover) = hover_after_edits("lsp-script-edit", &[&draft, SCRIPT], false) else {
        return;
    };
    assert!(hover["contents"].to_string().contains("fn from"), "{hover}");
}

#[cfg(target_os = "linux")]
#[test]
fn a_scripts_rust_analyzer_that_dies_starts_again_on_the_next_edit() {
    let Some(hover) = hover_after_edits("lsp-script-restart", &[SCRIPT], true) else {
        return;
    };
    assert!(hover["contents"].to_string().contains("fn from"), "{hover}");
}

#[test]
fn a_rust_analyzer_that_exits_at_once_starts_at_most_once_per_ten_seconds() {
    let dir = temp_dir("lsp-script-crash");
    let script = dir.join("tool.rk");
    fs::write(&script, SCRIPT).unwrap();
    let uri = format!("file://{}", script.display());
    // Each start adds a line to `starts`, and the process ends at once without an answer.
    let (mut server, mut stdin, received) =
        start(&dir, "#!/bin/sh\necho >> \"$RENK_TEST_DIR/starts\"\n");
    let started = Instant::now();
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
            "textDocument": {"uri": uri, "languageId": "renk", "version": 1, "text": SCRIPT},
        }}),
    );
    for version in 2..=20 {
        send(
            &mut stdin,
            json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
                "textDocument": {"uri": uri, "version": version},
                "contentChanges": [{"text": SCRIPT}],
            }}),
        );
    }

    // renk-lsp publishes a document's diagnostics after each sync.
    let mut published = 0;
    while published < 20 {
        let message = received.recv_timeout(Duration::from_secs(30)).unwrap();
        if message["method"] == "textDocument/publishDiagnostics" {
            published += 1;
        }
    }
    let starts = fs::read_to_string(dir.join("starts"))
        .unwrap_or_default()
        .lines()
        .count() as u64;
    let elapsed = started.elapsed();
    let _ = server.kill();
    let _ = server.wait();
    let _ = fs::remove_dir_all(&dir);
    // One start for the open, and at most one more for each ten seconds since.
    assert!(
        (1..=1 + elapsed.as_secs() / 10).contains(&starts),
        "{starts} starts in {elapsed:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_rust_analyzer_that_never_answers_initialize_is_stopped_after_one_wait() {
    let dir = temp_dir("lsp-script-silent");
    let script = dir.join("tool.rk");
    fs::write(&script, SCRIPT).unwrap();
    let uri = format!("file://{}", script.display());
    // Each start adds a line to `starts`, and the process reads its input but never answers.
    let silent = "#!/bin/sh\necho >> \"$RENK_TEST_DIR/starts\"\nwhile read -r line; do :; done\n";
    let (mut server, mut stdin, received) = start(&dir, silent);
    let started = Instant::now();
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
            "textDocument": {"uri": uri, "languageId": "renk", "version": 1, "text": SCRIPT},
        }}),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let silent = loop {
        if let Some(&silent) = rust_analyzer_children(server.id()).first() {
            break silent;
        }
        assert!(Instant::now() < deadline, "rust-analyzer did not start");
        thread::sleep(Duration::from_millis(20));
    };
    // renk-lsp publishes the opened document's diagnostics once it stops waiting for `initialize`.
    while received.recv_timeout(Duration::from_secs(30)).unwrap()["method"]
        != "textDocument/publishDiagnostics"
    {}

    // Each hover waits five seconds if renk-lsp still asks the silent rust-analyzer.
    let asked = Instant::now();
    for id in 2..5 {
        send(
            &mut stdin,
            json!({"jsonrpc": "2.0", "id": id, "method": "textDocument/hover", "params": {
                "textDocument": {"uri": uri},
                "position": {"line": 5, "character": 15},
            }}),
        );
        response(&received, id);
    }
    let hovers = asked.elapsed();
    let running = rust_analyzer_children(server.id());

    for version in 2..=20 {
        send(
            &mut stdin,
            json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
                "textDocument": {"uri": uri, "version": version},
                "contentChanges": [{"text": SCRIPT}],
            }}),
        );
    }
    let mut published = 0;
    while published < 19 {
        let message = received.recv_timeout(Duration::from_secs(30)).unwrap();
        if message["method"] == "textDocument/publishDiagnostics" {
            published += 1;
        }
    }
    let starts = fs::read_to_string(dir.join("starts"))
        .unwrap_or_default()
        .lines()
        .count() as u64;
    let elapsed = started.elapsed();
    let _ = server.kill();
    let _ = server.wait();
    let _ = fs::remove_dir_all(&dir);
    assert!(
        hovers < Duration::from_secs(5),
        "three hovers took {hovers:?}"
    );
    assert!(!running.contains(&silent), "{silent} still runs");
    // One start for the open, and at most one more for each ten seconds since.
    assert!(
        (1..=1 + elapsed.as_secs() / 10).contains(&starts),
        "{starts} starts in {elapsed:?}"
    );
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
    let mut running = rust_analyzer_children(server.id()).len();
    while running != 1 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(200));
        running = rust_analyzer_children(server.id()).len();
    }
    let _ = server.kill();
    let _ = server.wait();
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(running, 1);
}

#[cfg(target_os = "linux")]
#[test]
fn exit_ends_renk_lsp_and_its_rust_analyzers_while_stdin_stays_open() {
    if rust_analyzer().is_none() {
        return;
    }
    let dir = temp_dir("lsp-exit");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"tool\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let (mut server, mut stdin, received) = start(
        &dir,
        "#!/bin/sh\nexec \"$RENK_TEST_RUST_ANALYZER\" \"$@\"\n",
    );
    // The module goes to the workspace rust-analyzer, and the script to its own.
    for (path, text) in [
        ("src/main.rk", "fn main():\n    println!(\"hi\")\n"),
        ("tool.rk", SCRIPT),
    ] {
        send(
            &mut stdin,
            json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
                "textDocument": {
                    "uri": format!("file://{}", dir.join(path).display()),
                    "languageId": "renk",
                    "version": 1,
                    "text": text,
                },
            }}),
        );
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut started = rust_analyzer_children(server.id());
    while started.len() < 2 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
        started = rust_analyzer_children(server.id());
    }

    // Some editors send `shutdown` with `null` params.
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": null}),
    );
    let shutdown = loop {
        let message = received.recv_timeout(Duration::from_secs(30)).unwrap();
        if message["id"] == 2 {
            break message;
        }
    };
    send(&mut stdin, json!({"jsonrpc": "2.0", "method": "exit"}));
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut exited = server.try_wait().unwrap();
    while exited.is_none() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
        exited = server.try_wait().unwrap();
    }
    let running = started
        .iter()
        .filter(|pid| {
            fs::read_to_string(format!("/proc/{pid}/stat"))
                .is_ok_and(|stat| stat.contains("(rust-analyzer)") && !stat.contains(") Z "))
        })
        .collect::<Vec<_>>();
    let _ = server.kill();
    let _ = server.wait();
    for pid in &running {
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status();
    }
    drop(stdin);
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(started.len(), 2, "{started:?}");
    assert!(shutdown.get("error").is_none(), "{shutdown}");
    assert!(exited.is_some(), "renk-lsp still runs 2 s after exit");
    assert!(running.is_empty(), "{running:?} still run");
}
