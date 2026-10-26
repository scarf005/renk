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
