use std::{env, fs, path::PathBuf, process::Command};

fn temp_dir(name: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("rusk-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn script_runs_through_cargo_script_with_args_exit_code_and_source_path() {
    let dir = temp_dir("script-runner");
    let script = dir.join("echo-args.rsk");
    fs::write(
        &script,
        "#!/usr/bin/env -S rusk script\n---\n[package]\nedition = \"2024\"\n---\n\nfn main():\n    let args = std::env::args().skip(1).collect[Vec[String]]()\n    println!(\"{}\", args.join(\"|\"))\n    println!(\"{}\", std::env::var(\"RUSK_SCRIPT\").unwrap())\n    std::process::exit(3)\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rusk"))
        .args(["script", script.to_str().unwrap(), "a b", "--flag", "-x"])
        .env("XDG_CACHE_HOME", dir.join("cache"))
        .output()
        .unwrap();

    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "a b|--flag|-x\n{}\n",
            script.canonicalize().unwrap().display()
        ),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(3));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn script_reports_transpile_errors_with_the_script_path_and_line() {
    let dir = temp_dir("script-error");
    let script = dir.join("broken.rsk");
    fs::write(
        &script,
        "#!/usr/bin/env -S rusk script\nfn main():\n\tprintln!()\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rusk"))
        .args(["script", script.to_str().unwrap()])
        .env("XDG_CACHE_HOME", dir.join("cache"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("broken.rsk: line 3"), "{stderr}");
    assert!(!dir.join("cache").exists());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn script_no_cargo_execs_the_cached_binary_and_rebuilds_after_edits() {
    let dir = temp_dir("script-no-cargo");
    let script = dir.join("greet.rsk");
    let body = |greeting: &str| {
        format!(
            "#!/usr/bin/env -S rusk script --no-cargo\n---\n[package]\nedition = \"2024\"\n---\n\nfn main():\n    let args = std::env::args().skip(1).collect[Vec[String]]()\n    println!(\"{greeting} {{}}\", args.join(\"|\"))\n    println!(\"{{}}\", std::env::var(\"RUSK_SCRIPT\").unwrap())\n    std::process::exit(4)\n"
        )
    };
    let run = |path: &str| {
        Command::new(env!("CARGO_BIN_EXE_rusk"))
            .args([
                "script",
                "--no-cargo",
                script.to_str().unwrap(),
                "a b",
                "--flag",
            ])
            .env("XDG_CACHE_HOME", dir.join("cache"))
            .env("PATH", path)
            .output()
            .unwrap()
    };
    let expected = |greeting: &str| {
        format!(
            "{greeting} a b|--flag\n{}\n",
            script.canonicalize().unwrap().display()
        )
    };
    let path = env::var("PATH").unwrap();
    let no_cargo = dir.join("empty-path");
    fs::create_dir_all(&no_cargo).unwrap();

    fs::write(&script, body("hi")).unwrap();
    for path in [path.as_str(), no_cargo.to_str().unwrap()] {
        let output = run(path);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected("hi"),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.code(), Some(4));
    }

    fs::write(&script, body("bye")).unwrap();
    assert_ne!(run(no_cargo.to_str().unwrap()).status.code(), Some(4));
    let output = run(&path);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        expected("bye"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}
