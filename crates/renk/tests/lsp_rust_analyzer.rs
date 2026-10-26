#![cfg(unix)]

mod lsp;

use std::{
    fs,
    io::BufReader,
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use lsp::{read_message, rust_analyzer, send, temp_dir};
use serde_json::{Value, json};

/// `src/main.rk` of the test crate; `helper` comes from the Renk module `src/util.rk` and
/// `EXTRA` from the hand-written `src/extra.rs`.
const MAIN: &str = "mod extra\nmod util\nuse extra.EXTRA\nuse util.helper\n\nfn main():\n    let a = helper()\n    let b = helper() + EXTRA\n    println!(\"{a}{b}\")\n";

/// renk-lsp over a crate with `src/main.rk` open.
struct Session {
    server: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
    next_id: u64,
    dir: PathBuf,
}

impl Session {
    /// `None` without rust-analyzer.
    fn open(name: &str, main: &str) -> Option<Self> {
        rust_analyzer()?;
        let dir = temp_dir(name);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(dir.join("src/main.rk"), main).unwrap();
        fs::write(dir.join("src/util.rk"), "pub fn helper() -> i32 = 1\n").unwrap();
        fs::write(dir.join("src/extra.rs"), "pub const EXTRA: i32 = 2;\n").unwrap();

        let mut server = Command::new(env!("CARGO_BIN_EXE_renk-lsp"))
            .env("XDG_CACHE_HOME", dir.join("cache"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = server.stdin.take().unwrap();
        let mut stdout = BufReader::new(server.stdout.take().unwrap());
        let (sender, messages) = mpsc::channel();
        thread::spawn(move || {
            while let Some(message) = read_message(&mut stdout) {
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        let mut session = Self {
            server,
            stdin,
            messages,
            next_id: 1,
            dir,
        };
        let root = format!("file://{}", session.dir.display());
        session.request(
            "initialize",
            json!({"processId": null, "rootUri": root, "capabilities": {}}),
        );
        session.notify("initialized", json!({}));
        let uri = session.uri("src/main.rk");
        session.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "renk", "version": 1, "text": main}}),
        );
        Some(session)
    }

    fn uri(&self, path: &str) -> String {
        format!("file://{}", self.dir.join(path).display())
    }

    fn notify(&mut self, method: &str, params: Value) {
        send(
            &mut self.stdin,
            json!({"jsonrpc": "2.0", "method": method, "params": params}),
        );
    }

    /// The response to a request: its result, or its error.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        send(
            &mut self.stdin,
            json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
        );
        loop {
            let message = self.messages.recv_timeout(Duration::from_secs(30)).unwrap();
            if message["id"] == id && message.get("method").is_none() {
                return message.get("error").unwrap_or(&message["result"]).clone();
            }
        }
    }

    /// Repeats a request until `done` accepts its response, as rust-analyzer answers once it
    /// has loaded the crate, and returns the last response.
    fn request_until(
        &mut self,
        method: &str,
        params: Value,
        done: impl Fn(&Value) -> bool,
    ) -> Value {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let response = self.request(method, params.clone());
            if done(&response) || Instant::now() > deadline {
                return response;
            }
            let _ = self.messages.recv_timeout(Duration::from_millis(500));
        }
    }

    /// The diagnostics of the first publish for `uri` that `done` accepts, within two minutes.
    fn diagnostics_until(&mut self, uri: &str, done: impl Fn(&[Value]) -> bool) -> Vec<Value> {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let left = deadline
                .checked_duration_since(Instant::now())
                .expect("no such diagnostics arrived");
            let Ok(message) = self.messages.recv_timeout(left) else {
                continue;
            };
            if message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == uri
            {
                let diagnostics = message["params"]["diagnostics"].as_array().unwrap().clone();
                if done(&diagnostics) {
                    return diagnostics;
                }
            }
        }
    }

    /// The first cargo check errors of `uri` after saving `saved`, which is saved again every five
    /// seconds, as rust-analyzer checks on a save once it has loaded the crate.
    fn check_errors(&mut self, saved: &str, uri: &str) -> Vec<Value> {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            assert!(Instant::now() < deadline, "no cargo check errors arrived");
            self.notify(
                "textDocument/didSave",
                json!({"textDocument": {"uri": saved}}),
            );
            let waited = Instant::now() + Duration::from_secs(5);
            while let Some(left) = waited.checked_duration_since(Instant::now()) {
                let Ok(message) = self.messages.recv_timeout(left) else {
                    break;
                };
                if message["method"] != "textDocument/publishDiagnostics"
                    || message["params"]["uri"] != uri
                {
                    continue;
                }
                let diagnostics = message["params"]["diagnostics"].as_array().unwrap().clone();
                // rust-analyzer may report a type error too, but only cargo check is "rustc".
                let errors = diagnostics
                    .into_iter()
                    .filter(|diagnostic| {
                        diagnostic["severity"] == 1 && diagnostic["source"] == "rustc"
                    })
                    .collect::<Vec<_>>();
                if !errors.is_empty() {
                    return errors;
                }
            }
        }
    }

    fn change(&mut self, path: &str, version: i32, text: &str) {
        let uri = self.uri(path);
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": uri, "version": version},
                "contentChanges": [{"text": text}],
            }),
        );
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.server.kill();
        let _ = self.server.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn position(line: u32, character: u32) -> Value {
    json!({"line": line, "character": character})
}

fn range(line: u32, start: u32, end: u32) -> Value {
    json!({"start": position(line, start), "end": position(line, end)})
}

/// A workspace edit's edits of `uri`, in order.
fn edits_of(edit: &Value, uri: &str) -> Vec<Value> {
    let mut edits = edit["documentChanges"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|change| change["textDocument"]["uri"] == uri)
        .flat_map(|change| change["edits"].as_array().unwrap().clone())
        .chain(
            edit["changes"][uri]
                .as_array()
                .into_iter()
                .flatten()
                .cloned(),
        )
        .collect::<Vec<_>>();
    edits.sort_by_key(|edit| edit["range"]["start"]["line"].as_u64());
    edits
}

#[test]
fn cargo_check_type_errors_appear_at_the_renk_line_after_save() {
    let main = format!("{MAIN}    let wrong: u8 = \"text\"\n");
    let Some(mut session) = Session::open("lsp-check", &main) else {
        return;
    };
    let uri = session.uri("src/main.rk");
    let errors = session.check_errors(&uri, &uri);

    // The hand-written and Renk modules are in the mirror, so the type error is the only one.
    assert_eq!(errors.len(), 1, "{errors:?}");
    let error = &errors[0];
    assert!(
        error["message"].to_string().contains("mismatched types"),
        "{error}"
    );
    assert_eq!(error["range"], range(9, 20, 26), "{error}");
    assert_eq!(
        error["relatedInformation"][0]["location"],
        json!({"uri": uri, "range": range(9, 15, 17)}),
        "{error}"
    );
}

#[test]
fn cargo_check_on_save_reads_modules_changed_outside_the_editor() {
    let Some(mut session) = Session::open("lsp-check-disk", MAIN) else {
        return;
    };
    let (main, util) = (session.uri("src/main.rk"), session.uri("src/util.rk"));
    // rust-analyzer has started, so the mirror has been written.
    session.request_until(
        "textDocument/hover",
        json!({"textDocument": {"uri": main}, "position": position(6, 13)}),
        |hover| !hover.is_null(),
    );
    fs::write(
        session.dir.join("src/util.rk"),
        "pub fn helper() -> i32 = \"text\"\n",
    )
    .unwrap();
    let errors = session.check_errors(&main, &util);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0]["range"]["start"]["line"], 0, "{errors:?}");
}

#[test]
fn cargo_check_runs_on_saving_a_hand_written_rust_file() {
    let main = MAIN.replace("use extra.EXTRA\n", "use extra.EXTRA\nuse extra.new_api\n");
    let Some(mut session) = Session::open("lsp-check-rust", &main) else {
        return;
    };
    let (main, extra) = (session.uri("src/main.rk"), session.uri("src/extra.rs"));
    let errors = session.check_errors(&main, &main);
    assert!(
        errors[0]["message"].to_string().contains("new_api"),
        "{errors:?}"
    );

    fs::write(
        session.dir.join("src/extra.rs"),
        "pub const EXTRA: i32 = 2;\npub fn new_api() {}\n",
    )
    .unwrap();
    session.notify(
        "textDocument/didSave",
        json!({"textDocument": {"uri": extra}}),
    );
    session.diagnostics_until(&main, |diagnostics| {
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic["severity"] == 1 && diagnostic["source"] == "rustc")
    });
}

#[test]
fn references_map_to_renk_modules_open_or_on_disk() {
    let Some(mut session) = Session::open("lsp-references", MAIN) else {
        return;
    };
    let (main, util) = (session.uri("src/main.rk"), session.uri("src/util.rk"));
    let references = session.request_until(
        "textDocument/references",
        json!({
            "textDocument": {"uri": main},
            "position": position(6, 13),
            "context": {"includeDeclaration": true},
        }),
        |response| {
            response
                .as_array()
                .is_some_and(|locations| !locations.is_empty())
        },
    );

    let mut found = references
        .as_array()
        .unwrap_or_else(|| panic!("{references}"))
        .iter()
        .map(|location| {
            (
                location["uri"].as_str().unwrap().to_string(),
                location["range"].clone(),
            )
        })
        .collect::<Vec<_>>();
    found.sort_by_key(|(uri, range)| (uri.clone(), range["start"]["line"].as_u64()));
    assert_eq!(
        found,
        [
            (main.clone(), range(3, 9, 15)),
            (main.clone(), range(6, 12, 18)),
            (main, range(7, 12, 18)),
            (util, range(0, 7, 13)),
        ]
    );
}

#[test]
fn rename_edits_every_renk_use() {
    let Some(mut session) = Session::open("lsp-rename", MAIN) else {
        return;
    };
    let (main, util) = (session.uri("src/main.rk"), session.uri("src/util.rk"));
    let prepared = session.request_until(
        "textDocument/prepareRename",
        json!({"textDocument": {"uri": main}, "position": position(6, 13)}),
        |response| !response.is_null(),
    );
    assert_eq!(
        prepared.get("range").unwrap_or(&prepared),
        &range(6, 12, 18),
        "{prepared}"
    );

    let edit = session.request(
        "textDocument/rename",
        json!({"textDocument": {"uri": main}, "position": position(6, 13), "newName": "assist"}),
    );
    let renamed = |range: Value| json!({"range": range, "newText": "assist"});
    assert_eq!(
        edits_of(&edit, &main),
        [
            renamed(range(3, 9, 15)),
            renamed(range(6, 12, 18)),
            renamed(range(7, 12, 18))
        ],
        "{edit}"
    );
    assert_eq!(edits_of(&edit, &util), [renamed(range(0, 7, 13))], "{edit}");
}

#[test]
fn inlay_hints_land_at_renk_positions() {
    let Some(mut session) = Session::open("lsp-inlay", MAIN) else {
        return;
    };
    let main = session.uri("src/main.rk");
    let at = |hints: &Value| {
        hints.as_array().and_then(|hints| {
            hints
                .iter()
                .find(|hint| hint["position"] == position(6, 9))
                .cloned()
        })
    };
    let hints = session.request_until(
        "textDocument/inlayHint",
        json!({"textDocument": {"uri": main}, "range": {"start": position(0, 0), "end": position(9, 0)}}),
        |hints| at(hints).is_some(),
    );
    let hint = at(&hints).unwrap_or_else(|| panic!("{hints}"));
    assert!(hint["label"].to_string().contains("i32"), "{hint}");
}

#[test]
fn code_actions_edit_the_renk_document() {
    let Some(mut session) = Session::open("lsp-code-action", MAIN) else {
        return;
    };
    let main = session.uri("src/main.rk");
    let explicit = |actions: &Value| {
        actions.as_array().and_then(|actions| {
            actions
                .iter()
                .find(|action| action["title"].to_string().contains("explicit type"))
                .cloned()
        })
    };
    let actions = session.request_until(
        "textDocument/codeAction",
        json!({
            "textDocument": {"uri": main},
            "range": range(6, 8, 8),
            "context": {"diagnostics": []},
        }),
        |actions| explicit(actions).is_some(),
    );
    let action = explicit(&actions).unwrap_or_else(|| panic!("{actions}"));
    assert_eq!(
        edits_of(&action["edit"], &main),
        [
            json!({"range": {"start": position(6, 0), "end": position(7, 0)}, "newText": "    let a: i32 = helper()\n"})
        ],
        "{action}"
    );
}

#[test]
fn code_actions_edit_a_scripts_top_level_lines() {
    let script = "#!/usr/bin/env -S renk script\nlet a = 1 + 1\nprintln!(\"{a}\")\n";
    let Some(mut session) = Session::open("lsp-script-code-action", script) else {
        return;
    };
    let main = session.uri("src/main.rk");
    let explicit = |actions: &Value| {
        actions.as_array().and_then(|actions| {
            actions
                .iter()
                .find(|action| action["title"].to_string().contains("explicit type"))
                .cloned()
        })
    };
    let actions = session.request_until(
        "textDocument/codeAction",
        json!({
            "textDocument": {"uri": main},
            "range": range(1, 4, 4),
            "context": {"diagnostics": []},
        }),
        |actions| explicit(actions).is_some(),
    );
    let action = explicit(&actions).unwrap_or_else(|| panic!("{actions}"));
    assert_eq!(
        edits_of(&action["edit"], &main),
        [
            json!({"range": {"start": position(1, 0), "end": position(2, 0)}, "newText": "let a: i32 = 1 + 1\n"})
        ],
        "{action}"
    );
}

#[test]
fn rust_analyzer_diagnostics_go_while_the_renk_text_does_not_transpile() {
    let main = format!("{MAIN}    let wrong: u8 = \"text\"\n");
    let Some(mut session) = Session::open("lsp-stale-diagnostics", &main) else {
        return;
    };
    let uri = session.uri("src/main.rk");
    let from = |source: &'static str| {
        move |diagnostics: &[Value]| diagnostics.iter().any(|d| d["source"] == source)
    };
    let found = session.diagnostics_until(&uri, from("rust-analyzer"));
    let error = found.iter().find(|d| d["source"] == "rust-analyzer");
    assert_eq!(error.unwrap()["range"]["start"]["line"], 9, "{found:?}");

    // A tab is a Renk syntax error, and the line it adds would move rust-analyzer's error.
    session.change("src/main.rk", 2, &format!("\t\n{main}"));
    let broken = session.diagnostics_until(&uri, from("renk"));
    assert!(broken.iter().all(|d| d["source"] == "renk"), "{broken:?}");

    session.change("src/main.rk", 3, &main);
    let fixed = session.diagnostics_until(&uri, from("rust-analyzer"));
    assert!(fixed.iter().all(|d| d["source"] != "renk"), "{fixed:?}");
    assert!(
        fixed
            .iter()
            .all(|d| d["source"] != "rust-analyzer" || d["range"]["start"]["line"] == 9),
        "{fixed:?}"
    );
}
