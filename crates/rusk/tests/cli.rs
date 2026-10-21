use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

fn temp_dir(name: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("rusk-cli-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn rusk(args: &[&str], stdin: &str, cwd: &Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rusk"))
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
    let output = rusk(&[], "fn main():\n    println!(\"hi\")\n", &dir);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        "fn main() {\n    println!(\"hi\");\n}\n"
    );
    assert!(output.stderr.is_empty());
    let _ = fs::remove_dir_all(dir);
}

#[cfg(target_os = "linux")]
#[test]
fn transpiles_when_the_process_limit_refuses_threads() {
    // With no thread to spare, the conversion runs on the caller's stack.
    let dir = temp_dir("process-limit");
    fs::write(dir.join("main.rsk"), "fn main():\n    println!(\"hi\")\n").unwrap();
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "ulimit -u 1 && exec '{}' transpile main.rsk",
            env!("CARGO_BIN_EXE_rusk")
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
        dir.join("main.rsk"),
        "fn main():\n    let x = 1\n    println!(\"{x}\")\n",
    )
    .unwrap();
    let output = rusk(
        &[
            "transpile",
            "main.rsk",
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
    fs::write(dir.join("bad.rsk"), "fn main():\n\tx\n").unwrap();
    let cases: [(&[&str], &str); 9] = [
        (&["bad.rsk"], "rusk: line 2: tabs are not supported"),
        (&["missing.rsk"], "rusk: "),
        (&["--bogus"], "rusk: unknown option: --bogus"),
        (
            &["a.rsk", "b.rsk"],
            "rusk: only one input path is supported",
        ),
        (&["bad.rsk", "-o"], "rusk: --output requires a path"),
        (
            &["fmt", "bad.rsk", "--line-width", "wide"],
            "rusk: --line-width requires a number",
        ),
        (
            &["fmt", "bad.rsk", "--source-map", "x.json"],
            "rusk: --source-map is not supported with fmt",
        ),
        (
            &["from-rust", "bad.rsk", "--source-map", "x.json"],
            "rusk: --source-map is not supported with syntax conversion",
        ),
        (
            &["fmt", "bad.rsk", "--aggressive"],
            "rusk: --aggressive is only supported with from-rust",
        ),
    ];
    for (args, expected) in cases {
        let output = rusk(args, "", &dir);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
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
    let output = rusk(&["fmt", "--line-width", "0"], "fn main() = ()\n", &dir);
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("line width must be greater than zero"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn help_exits_successfully() {
    let dir = temp_dir("help");
    for flag in ["-h", "--help"] {
        let output = rusk(&[flag], "", &dir);
        assert_eq!(output.status.code(), Some(0));
        assert!(
            text(&output.stdout).contains("USAGE:"),
            "{}",
            text(&output.stdout)
        );
    }
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
        dir.join("a.rsk"),
        "fn f() -> i32:\n    let x = 1  \n    x\n",
    )
    .unwrap();
    let cases: [(&[&str], &str); 3] = [
        (
            &["from-rust", "a.rs"],
            "fn f() -> i32:\n    let x = 1\n    x\n",
        ),
        (&["fmt", "a.rsk"], "fn f() -> i32:\n    let x = 1\n    x\n"),
        (
            &["from-rust", "--aggressive", "b.rs"],
            "fn f(c: bool) -> i32 = if c then 1 else 2\n",
        ),
    ];
    for (args, expected) in cases {
        let output = rusk(args, "", &dir);
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
        dir.join("src/main.rsk"),
        "fn main():\n    println!(\"wrapped\")\n",
    )
    .unwrap();
    let target = dir.join("target");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rusk"))
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
            .starts_with("// @generated by rusk")
    );

    fs::write(dir.join("src/main.rsk"), "fn main():\n    missing()\n").unwrap();
    let output = run(&["cargo", "check", "-q"]);
    assert_eq!(output.status.code(), Some(101), "{}", text(&output.stderr));
    assert!(
        text(&output.stderr).contains("missing"),
        "{}",
        text(&output.stderr)
    );

    fs::write(dir.join("src/main.rsk"), "fn main():\n\tx\n").unwrap();
    let output = run(&["check"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).starts_with("rusk: line 2:"),
        "{}",
        text(&output.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}
