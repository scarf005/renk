use std::{
    env, fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

fn temp_dir(name: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("rusk-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn send(stdin: &mut impl Write, message: Value) {
    let body = message.to_string();
    write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    stdin.flush().unwrap();
}

fn read_message(reader: &mut impl BufRead) -> Option<Value> {
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

#[test]
fn script_hover_comes_from_rust_analyzer_over_the_scripts_generated_rust() {
    if Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipped: rust-analyzer is not installed");
        return;
    }
    let dir = temp_dir("lsp-script");
    let script = dir.join("tool.rsk");
    let text = "#!/usr/bin/env -S rusk script\n---\n[package]\nedition = \"2024\"\n---\nlet name = String.from(\"a\")\nprintln!(\"{name}\")\n";
    fs::write(&script, text).unwrap();
    let uri = format!("file://{}", script.display());

    let mut server = Command::new(env!("CARGO_BIN_EXE_rusk-lsp"))
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
            "textDocument": {"uri": uri, "languageId": "rusk", "version": 1, "text": text},
        }}),
    );

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
        if hover["contents"].to_string().contains("fn from")
            || Instant::now() > deadline
        {
            break hover;
        }
        id += 1;
        thread::sleep(Duration::from_millis(500));
    };
    let _ = server.kill();
    let _ = server.wait();
    let _ = fs::remove_dir_all(&dir);

    assert!(hover["contents"].to_string().contains("fn from"), "{hover}");
    assert_eq!(hover["range"]["start"]["line"], 5, "{hover}");
}
