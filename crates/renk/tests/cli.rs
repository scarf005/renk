use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

fn temp_dir(name: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("renk-cli-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn renk(args: &[&str], stdin: &str, cwd: &Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_renk"))
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn transpiles_stdin_to_stdout() {
    let dir = temp_dir("stdin");
    let output = renk(&[], "fn main():\n    println!(\"hi\")\n", &dir);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        "fn main() {\n    println!(\"hi\");\n}\n"
    );
    assert!(output.stderr.is_empty());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn transpiles_a_script_as_renk_script_builds_it() {
    let dir = temp_dir("script");
    let script = "#!/usr/bin/env -S renk script\n---\n[dependencies]\nregex = \"1\"\n---\nprintln!(\"hi\")\n";
    fs::write(dir.join("tool.rk"), script).unwrap();
    let output = renk(&["transpile", "tool.rk"], "", &dir);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let rust = text(&output.stdout);
    assert!(
        rust.starts_with("---\n[dependencies]\nregex = \"1\"\n"),
        "{rust}"
    );
    assert!(!rust.contains("#!"), "{rust}");
    assert!(
        rust.contains("fn main() -> Result<(), Box<dyn std::error::Error>> {"),
        "{rust}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[cfg(target_os = "linux")]
#[test]
fn transpiles_when_the_process_limit_refuses_threads() {
    // With no thread to spare, the conversion runs on the caller's stack.
    let dir = temp_dir("process-limit");
    fs::write(dir.join("main.rk"), "fn main():\n    println!(\"hi\")\n").unwrap();
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "ulimit -u 1 && exec '{}' transpile main.rk",
            env!("CARGO_BIN_EXE_renk")
        ))
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        "fn main() {\n    println!(\"hi\");\n}\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn writes_output_and_a_json_source_map() {
    let dir = temp_dir("source-map");
    fs::write(
        dir.join("main.rk"),
        "fn main():\n    let x = 1\n    println!(\"{x}\")\n",
    )
    .unwrap();
    let output = renk(
        &[
            "transpile",
            "main.rk",
            "-o",
            "main.rs",
            "--source-map",
            "main.map.json",
        ],
        "",
        &dir,
    );
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty());
    let rust = fs::read_to_string(dir.join("main.rs")).unwrap();
    assert!(rust.contains("    let x = 1;\n"), "{rust}");
    let map: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("main.map.json")).unwrap()).unwrap();
    assert!(map.is_object(), "{map}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn reports_errors_on_stderr_with_exit_code_one() {
    let dir = temp_dir("errors");
    fs::write(dir.join("bad.rk"), "fn main():\n\tx\n").unwrap();
    let cases: [(&[&str], i32, &str); 8] = [
        (
            &["transpile", "bad.rk"],
            1,
            "renk: line 2: tabs are not supported",
        ),
        (&["transpile", "missing.rk"], 1, "renk: "),
        (
            &["missing.rk"],
            1,
            "renk: not a subcommand or script file: missing.rk",
        ),
        (
            &["transpile", "--bogus"],
            2,
            "error: unexpected argument '--bogus'",
        ),
        (
            &["transpile", "a.rk", "b.rk"],
            2,
            "error: unexpected argument 'b.rk'",
        ),
        (
            &["transpile", "bad.rk", "-o"],
            2,
            "error: a value is required for '--output <PATH>'",
        ),
        (
            &["fmt", "bad.rk", "--source-map", "x.json"],
            2,
            "error: unexpected argument '--source-map'",
        ),
        (
            &["fmt", "bad.rk", "--aggressive"],
            2,
            "error: unexpected argument '--aggressive'",
        ),
    ];
    for (args, code, expected) in cases {
        let output = renk(args, "", &dir);
        assert_eq!(output.status.code(), Some(code), "{args:?}");
        assert!(
            output.stdout.is_empty(),
            "{args:?}: {}",
            text(&output.stdout)
        );
        assert!(
            text(&output.stderr).starts_with(expected),
            "{args:?}: {}",
            text(&output.stderr)
        );
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn help_exits_successfully() {
    let dir = temp_dir("help");
    for flag in ["-h", "--help"] {
        let output = renk(&[flag], "", &dir);
        assert_eq!(output.status.code(), Some(0));
        assert!(
            text(&output.stdout).contains("Usage:"),
            "{}",
            text(&output.stdout)
        );
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn subcommand_help_describes_its_arguments() {
    let dir = temp_dir("subcommand-help");
    for (command, expected) in [
        ("script", "--cargo"),
        ("add", "cargo add"),
        ("from-rust", "--aggressive"),
    ] {
        let output = renk(&[command, "--help"], "", &dir);
        assert_eq!(output.status.code(), Some(0), "{command}");
        assert!(
            text(&output.stdout).contains(expected),
            "{command}: {}",
            text(&output.stdout)
        );
    }
    let top = text(&renk(&["--help"], "", &dir).stdout);
    assert!(!top.contains("RENK_SCRIPT"), "{top}");
    assert!(top.lines().count() < 25, "{top}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn subcommands_dispatch_on_the_input_extension() {
    let dir = temp_dir("dispatch");
    fs::write(
        dir.join("a.rs"),
        "fn f() -> i32 {\n    let x = 1;\n    x\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("b.rs"),
        "fn f(c: bool) -> i32 {\n    if c {\n        1\n    } else {\n        2\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("a.rk"),
        "fn f() -> i32:\n    let x = 1  \n    x\n",
    )
    .unwrap();
    let cases: [(&[&str], &str); 3] = [
        (
            &["from-rust", "a.rs"],
            "fn f() -> i32:\n    let x = 1\n    x\n",
        ),
        (&["fmt", "a.rk"], "fn f() -> i32:\n    let x = 1\n    x\n"),
        (
            &["from-rust", "--aggressive", "b.rs"],
            "fn f(c: bool) -> i32 = if c then 1 else 2\n",
        ),
    ];
    for (args, expected) in cases {
        let output = renk(args, "", &dir);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{args:?}: {}",
            text(&output.stderr)
        );
        assert_eq!(text(&output.stdout), expected, "{args:?}");
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cargo_wrapper_generates_sources_and_forwards_cargo_exit_codes() {
    let dir = temp_dir("cargo");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"wrapped\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
    )
    .unwrap();
    fs::write(
        dir.join("src/main.rk"),
        "fn main():\n    println!(\"wrapped\")\n",
    )
    .unwrap();
    let target = dir.join("target");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_renk"))
            .args(args)
            .current_dir(&dir)
            .env("CARGO_TARGET_DIR", &target)
            .output()
            .unwrap()
    };

    let output = run(&["run", "-q"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "wrapped\n");
    assert!(
        fs::read_to_string(dir.join("src/main.rs"))
            .unwrap()
            .starts_with("// @generated by renk")
    );

    fs::write(dir.join("src/main.rk"), "fn main():\n    missing()\n").unwrap();
    let output = run(&["cargo", "check", "-q"]);
    assert_eq!(output.status.code(), Some(101), "{}", text(&output.stderr));
    assert!(
        text(&output.stderr).contains("missing"),
        "{}",
        text(&output.stderr)
    );

    fs::write(dir.join("src/main.rk"), "fn main():\n\tx\n").unwrap();
    let output = run(&["check"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).starts_with("renk: line 2:"),
        "{}",
        text(&output.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn fmt_check_prints_a_diff_and_fails_for_unformatted_files() {
    let dir = temp_dir("fmt-check");
    let formatted = "fn main():\n    let x = 1\n    println!(\"{x}\")\n";
    let unformatted = "fn main():\n  let  x = 1\n  println!(\"{x}\")\n";
    fs::write(dir.join("good.rk"), formatted).unwrap();
    fs::write(dir.join("bad.rk"), unformatted).unwrap();

    let output = renk(&["fmt", "--check", "good.rk"], "", &dir);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));

    let output = renk(&["fmt", "--check", "good.rk", "bad.rk"], "", &dir);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        "--- bad.rk\n+++ bad.rk\n@@ -1,3 +1,3 @@\n fn main():\n-  let  x = 1\n-  println!(\"{x}\")\n+    let x = 1\n+    println!(\"{x}\")\n"
    );
    assert_eq!(fs::read_to_string(dir.join("bad.rk")).unwrap(), unformatted);

    let output = renk(&["fmt", "--check"], "fn main():\n  f()\n", &dir);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert!(
        text(&output.stdout).starts_with("--- <stdin>\n+++ <stdin>\n"),
        "{}",
        text(&output.stdout)
    );

    let output = renk(&["fmt", "bad.rk", "-o", "out.rk"], "", &dir);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(fs::read_to_string(dir.join("out.rk")).unwrap(), formatted);

    let output = renk(&["fmt", "good.rk", "bad.rk"], "", &dir);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        text(&output.stderr),
        "renk: formatting more than one file needs --check\n"
    );
    let output = renk(&["fmt", "--check", "good.rk", "-o", "out.rk"], "", &dir);
    assert_eq!(output.status.code(), Some(2));

    let output = Command::new(env!("CARGO_BIN_EXE_renk"))
        .args(["fmt", "good.rk"])
        .current_dir(&dir)
        .env("PATH", &dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).starts_with("renk: cannot run rustfmt"),
        "{}",
        text(&output.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}

/// A Cargo project with `src/main.rk` and a hand-written `src/other.rs` that fails to compile.
fn cargo_project(name: &str, main: &str) -> PathBuf {
    let dir = temp_dir(name);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"wrapped\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
    )
    .unwrap();
    fs::write(dir.join("src/main.rk"), main).unwrap();
    fs::write(dir.join("src/other.rs"), "pub fn f() -> i32 { \"x\" }\n").unwrap();
    dir
}

fn cargo_wrapper(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_renk"))
        .args(args)
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", dir.join("target"))
        .env("RUST_BACKTRACE", "1")
        .output()
        .unwrap()
}

const UNRESOLVED: &str = "mod other\n\nfn main():\n    let config = 1\n    let a = String.nope()\n    let b = config.MAX\n    let c: String = config.count_ones()\n    let y: i32 = \"nope\"\n";

#[test]
fn cargo_wrapper_diagnostics_point_at_renk_lines_and_columns() {
    let dir = cargo_project("cargo-diagnostics", UNRESOLVED);
    let output = cargo_wrapper(&dir, &["check"]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(101), "{stderr}");
    for expected in [
        "--> src/main.rk:5:20\n",
        "5 |     let a = String.nope()\n",
        "--> src/main.rk:6:20\n",
        "--> src/main.rk:7:28\n",
        "--> src/main.rk:8:18\n",
        "--> src/other.rs:1:21\n",
        "could not compile `wrapped`",
    ] {
        assert!(stderr.contains(expected), "{expected}\n{stderr}");
    }
    assert!(!stderr.contains("main.rs"), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cargo_wrapper_maps_json_and_short_messages_to_renk_sources() {
    let dir = cargo_project("cargo-json", UNRESOLVED);
    let output = cargo_wrapper(&dir, &["cargo", "check", "--message-format", "json"]);
    assert_eq!(output.status.code(), Some(101), "{}", text(&output.stderr));
    let messages = text(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message["reason"] == "build-finished")
    );
    let diagnostic = |code: &str| {
        messages
            .iter()
            .map(|message| &message["message"])
            .find(|message| message["code"]["code"] == code)
            .unwrap_or_else(|| panic!("no {code} in {messages:?}"))
    };
    let unresolved = diagnostic("E0599");
    let span = &unresolved["spans"][0];
    assert_eq!(span["file_name"], "src/main.rk");
    assert_eq!(
        (span["line_start"].as_u64(), span["column_start"].as_u64()),
        (Some(5), Some(20))
    );
    assert_eq!(span["text"][0]["text"], "    let a = String.nope()");
    assert_eq!(span["text"][0]["highlight_start"], 20);
    let rendered = unresolved["rendered"].as_str().unwrap();
    assert!(rendered.contains("--> src/main.rk:5:20\n"), "{rendered}");
    let mismatched = messages
        .iter()
        .map(|message| &message["message"])
        .filter(|message| message["code"]["code"] == "E0308")
        .collect::<Vec<_>>();
    let rk = mismatched
        .iter()
        .find(|message| message["spans"][0]["file_name"] == "src/main.rk")
        .unwrap();
    let mut spans = vec![];
    let mut stack = vec![*rk];
    while let Some(message) = stack.pop() {
        spans.extend(message["spans"].as_array().unwrap());
        stack.extend(message["children"].as_array().unwrap());
    }
    assert!(
        spans
            .iter()
            .all(|span| span["file_name"] == "src/main.rk"
                && span["suggested_replacement"].is_null()),
        "{spans:?}"
    );
    assert!(
        mismatched
            .iter()
            .any(|message| message["spans"][0]["file_name"] == "src/other.rs"),
        "{mismatched:?}"
    );

    let output = cargo_wrapper(&dir, &["check", "--message-format=short"]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(101), "{stderr}");
    assert!(
        stderr.contains(
            "src/main.rk:5:20: error[E0599]: no associated function or constant named `nope` found for struct `String` in the current scope\n"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains("src/other.rs:1:21: error[E0308]: mismatched types"),
        "{stderr}"
    );
    assert!(!stderr.contains("-->"), "{stderr}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cargo_wrapper_rewrites_panic_locations_of_run_and_test() {
    let main = "fn main():\n    let v: Vec[i32] = Vec.new()\n    println!(\"{v[3]}\")\n\n#[cfg(test)]\nmod tests:\n    #[test]\n    fn fails():\n        assert_eq!(1 + 1, 3)\n";
    let dir = cargo_project("cargo-panic", main);
    fs::remove_file(dir.join("src/other.rs")).unwrap();
    // rustc puts an index's panic at its `[`.
    let column = "    println!(\"{v[3]}\")".find('[').unwrap() + 1;

    let output = cargo_wrapper(&dir, &["run", "-q"]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(101), "{stderr}");
    assert!(
        stderr.contains(&format!("panicked at src/main.rk:3:{column}:\n")),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!("at src/main.rk:3:{column}\n")),
        "{stderr}"
    );
    assert!(!stderr.contains("main.rs"), "{stderr}");

    let output = cargo_wrapper(&dir, &["test", "-q"]);
    let stdout = text(&output.stdout);
    assert_eq!(output.status.code(), Some(101), "{stdout}");
    assert!(
        stdout.contains("panicked at src/main.rk:9:9:\n"),
        "{stdout}"
    );
    assert!(!stdout.contains("main.rs"), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cargo_wrapper_run_shows_a_prompt_without_its_newline() {
    let main = "use std.io.Write\n\nfn main():\n    print!(\"name? \")\n    std.io.stdout().flush().unwrap()\n    let mut line = String.new()\n    std.io.stdin().read_line(&mut line).unwrap()\n    println!(\"hi {line.trim()}\")\n";
    let dir = cargo_project("cargo-prompt", main);
    fs::remove_file(dir.join("src/other.rs")).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_renk"))
        .args(["run", "-q"])
        .current_dir(&dir)
        .env("CARGO_TARGET_DIR", dir.join("target"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = read_until(child.stdout.take().unwrap(), "name? ");
    child.stdin.take().unwrap().write_all(b"Ada\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(stdout.collect::<String>(), "hi Ada\n");
    let _ = fs::remove_dir_all(dir);
}

/// Reads `stream` until its text so far is `expected`, failing after two minutes, and returns the
/// text that follows, until the stream ends.
fn read_until(
    mut stream: impl std::io::Read + Send + 'static,
    expected: &str,
) -> impl Iterator<Item = String> {
    let (sender, chunks) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buffer = [0; 1024];
        while let Ok(read @ 1..) = stream.read(&mut buffer) {
            if sender.send(text(&buffer[..read])).is_err() {
                break;
            }
        }
    });
    let mut read = String::new();
    while read != expected {
        read += &chunks
            .recv_timeout(std::time::Duration::from_secs(120))
            .unwrap_or_else(|_| panic!("no {expected:?} after {read:?}"));
        assert!(expected.starts_with(&read), "{read:?}");
    }
    chunks.into_iter()
}

#[cfg(target_os = "linux")]
#[test]
fn cargo_wrapper_keeps_terminals_for_the_program_and_libtest_colors() {
    use std::{
        io::Read,
        os::fd::{FromRawFd, OwnedFd},
    };

    let main = "use std.io.IsTerminal\n\nfn main():\n    println!(\"out {std.io.stdout().is_terminal()}\")\n    eprintln!(\"err {std.io.stderr().is_terminal()}\")\n    let v: Vec[i32] = Vec.new()\n    println!(\"{v[3]}\")\n\n#[cfg(test)]\nmod tests:\n    #[test]\n    fn fails():\n        assert_eq!(1 + 1, 3)\n";
    let dir = cargo_project("cargo-terminal", main);
    fs::remove_file(dir.join("src/other.rs")).unwrap();
    // The output of renk run with its stdout and stderr on a terminal.
    let on_terminal = |args: &[&str]| {
        let (mut parent, mut child) = (0, 0);
        // SAFETY: `openpty` gets valid pointers and opens two descriptors only the files own.
        let (mut parent, child) = unsafe {
            let opened = libc::openpty(
                &mut parent,
                &mut child,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert_eq!(opened, 0);
            (fs::File::from_raw_fd(parent), OwnedFd::from_raw_fd(child))
        };
        let mut renk = Command::new(env!("CARGO_BIN_EXE_renk"))
            .args(args)
            .current_dir(&dir)
            .env("CARGO_TARGET_DIR", dir.join("target"))
            // libtest colors only a terminal that `TERM` names.
            .env("TERM", "xterm-256color")
            .stdin(Stdio::null())
            .stdout(child.try_clone().unwrap())
            .stderr(child)
            .spawn()
            .unwrap();
        let mut output = Vec::new();
        // The terminal fails to read once renk and cargo close it.
        let _ = parent.read_to_end(&mut output);
        renk.wait().unwrap();
        text(&output).replace("\r\n", "\n")
    };

    let output = on_terminal(&["run", "-q"]);
    assert!(output.contains("out true\n"), "{output}");
    assert!(output.contains("err true\n"), "{output}");
    assert!(output.contains("panicked at src/main.rk:7:"), "{output}");
    assert!(!output.contains("main.rs"), "{output}");
    assert!(!output.contains("\"reason\""), "{output}");

    let output = on_terminal(&["test", "-q"]);
    assert!(
        output.contains("panicked at src/main.rk:13:9:\n"),
        "{output}"
    );
    assert!(output.contains("\u{1b}[31mFAILED"), "{output}");
    let _ = fs::remove_dir_all(dir);
}
