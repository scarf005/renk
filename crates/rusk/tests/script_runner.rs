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
fn scripts_share_one_build_dir_and_each_runs_its_own_binary() {
    let dir = temp_dir("script-shared-build");
    let run = |script: &PathBuf| {
        Command::new(env!("CARGO_BIN_EXE_rusk"))
            .args(["script", script.to_str().unwrap()])
            .env("XDG_CACHE_HOME", dir.join("cache"))
            .output()
            .unwrap()
    };
    // Same file stems, and scripts naming the same package, build the same root unit names.
    let scripts = [
        ("a", "tool", ""),
        ("b", "tool", ""),
        ("c", "named", "name = \"same\"\n"),
        ("d", "named", "name = \"same\"\n"),
    ]
    .map(|(parent, stem, name)| {
        fs::create_dir_all(dir.join(parent)).unwrap();
        let script = dir.join(parent).join(format!("{stem}.rsk"));
        fs::write(
            &script,
            format!("---\n[package]\n{name}edition = \"2024\"\n---\nprintln!(\"{parent}\")\n"),
        )
        .unwrap();
        (parent, script)
    });

    for (parent, script) in scripts.iter().chain(&scripts) {
        let output = run(script);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("{parent}\n"),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(dir.join("cache/rusk/build").is_dir());
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

#[test]
fn script_without_an_edition_runs_without_the_cargo_edition_warning() {
    let dir = temp_dir("script-edition");
    for (name, header) in [("bare", ""), ("listed", "---\n[dependencies]\n---\n")] {
        let script = dir.join(format!("{name}.rsk"));
        fs::write(
            &script,
            format!("{header}fn main():\n    println!(\"{name}\")\n"),
        )
        .unwrap();
        for mode in [None, Some("--no-cargo")] {
            let output = Command::new(env!("CARGO_BIN_EXE_rusk"))
                .arg("script")
                .args(mode)
                .arg(&script)
                .env("XDG_CACHE_HOME", dir.join("cache"))
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                format!("{name}\n"),
                "{stderr}"
            );
            assert!(!stderr.contains("edition"), "{stderr}");
        }
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn script_compiler_diagnostics_point_at_script_lines() {
    let dir = temp_dir("script-diagnostics");
    let broken = dir.join("broken.rsk");
    fs::write(
        &broken,
        "#!/usr/bin/env -S rusk script\n---\n[package]\nedition = \"2024\"\n---\n\nstruct Point:\n    x: i32\n\nfn main():\n    let p = Point{ x: 1 }\n    let y: i32 = \"nope\"\n    println!(\"{p.x} {y}\")\n",
    )
    .unwrap();
    let warned = dir.join("warned.rsk");
    fs::write(
        &warned,
        "fn main():\n\n    let unused = 1\n    eprintln!(\"broken.rs:1:1\")\n",
    )
    .unwrap();
    for mode in [None, Some("--no-cargo")] {
        let run = |script: &PathBuf| {
            Command::new(env!("CARGO_BIN_EXE_rusk"))
                .arg("script")
                .args(mode)
                .arg(script)
                .env("XDG_CACHE_HOME", dir.join("cache"))
                .output()
                .unwrap()
        };
        let output = run(&broken);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_ne!(output.status.code(), Some(0), "{stderr}");
        assert!(
            stderr.contains(&format!("--> {}:12:", broken.display())),
            "{stderr}"
        );
        assert!(
            stderr.contains("\n12 |     let y: i32 = \"nope\"\n"),
            "{stderr}"
        );
        assert!(!stderr.contains("broken.rs:"), "{stderr}");

        let output = run(&warned);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(0), "{stderr}");
        if mode.is_none() {
            assert!(
                stderr.contains(&format!("--> {}:3:", warned.display())),
                "{stderr}"
            );
        }
        assert!(stderr.ends_with("broken.rs:1:1\n"), "{stderr}");
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn script_top_level_statements_run_in_a_fallible_main() {
    let dir = temp_dir("script-top-level");
    let run = |name: &str, source: &str| {
        let script = dir.join(name);
        fs::write(&script, source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_rusk"))
            .arg("script")
            .arg(&script)
            .env("XDG_CACHE_HOME", dir.join("cache"))
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        (output.status.code(), stdout, stderr, script)
    };

    let (code, stdout, stderr, _) = run(
        "tool.rsk",
        "#!/usr/bin/env -S rusk script\n#![allow(dead_code)]\nuse std.collections.HashMap\n\nstruct Count:\n    n: i32\n\nlet text = r#\"\nfn main():\n    nested\n\"#\nlet n: i32 = \"4\".parse()?\nlet mut seen = HashMap.new()\nfor i in 0..n:\n    seen.insert(i, Count{ n: i * 2 })\nprintln!(\"{} {} {}\", seen.len(), seen[&3].n, text.lines().count())\n",
    );
    assert_eq!(stdout, "4 6 3\n", "{stderr}");
    assert_eq!(code, Some(0), "{stderr}");

    let (code, stdout, stderr, _) = run(
        "fail.rsk",
        "let n: i32 = \"x\".parse()?\nprintln!(\"{n}\")\n",
    );
    assert_eq!(stdout, "", "{stderr}");
    assert_eq!(code, Some(1), "{stderr}");
    assert!(stderr.contains("InvalidDigit"), "{stderr}");

    let (code, stdout, stderr, _) = run("tool.rk", "let x = 2\nprintln!(\"{}\", x * 3)\n");
    assert_eq!(stdout, "6\n", "{stderr}");
    assert_eq!(code, Some(0), "{stderr}");

    let (code, _, stderr, script) = run(
        "typed.rsk",
        "let x = 1\nlet y: i32 = \"no\"\nprintln!(\"{x}\")\n",
    );
    assert_ne!(code, Some(0), "{stderr}");
    assert!(
        stderr.contains(&format!("--> {}:2:", script.display())),
        "{stderr}"
    );

    let (code, _, stderr, _) = run("tabbed.rsk", "let x = 1\nif x > 0:\n\tprintln!()\n");
    assert_eq!(code, Some(1), "{stderr}");
    assert!(stderr.contains("tabbed.rsk: line 3"), "{stderr}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn script_warnings_print_only_when_the_script_compiles() {
    let dir = temp_dir("script-fresh-warnings");
    let script = dir.join("warned.rsk");
    let run = || {
        let output = Command::new(env!("CARGO_BIN_EXE_rusk"))
            .arg("script")
            .arg(&script)
            .env("XDG_CACHE_HOME", dir.join("cache"))
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
        String::from_utf8_lossy(&output.stderr).into_owned()
    };
    fs::write(
        &script,
        "fn main():\n    let unused = 1\n    println!(\"1\")\n",
    )
    .unwrap();
    assert!(run().starts_with("warning: unused variable"));
    assert_eq!(run(), "");
    fs::write(
        &script,
        "fn main():\n    let unused = 2\n    println!(\"1\")\n",
    )
    .unwrap();
    assert!(run().contains("unused variable"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn script_diagnostics_show_script_columns_and_source() {
    let dir = temp_dir("script-columns");
    let script = dir.join("columns.rsk");
    fs::write(
        &script,
        "struct P:\n    x: i32\n\nfn main():\n    let p = P{ x: 1 }\n    let v: Vec[String] = Vec.new()\n    println!(\"{p.nope()} {v.len()}\")\n    let s: Vec[u8] = String.nope()\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rusk"))
        .arg("script")
        .arg(&script)
        .env("XDG_CACHE_HOME", dir.join("cache"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_ne!(output.status.code(), Some(0), "{stderr}");
    for (line, code) in [
        (7, "    println!(\"{p.nope()} {v.len()}\")"),
        (8, "    let s: Vec[u8] = String.nope()"),
    ] {
        let column = code.find("nope").unwrap();
        assert!(
            stderr.contains(&format!("--> {}:{line}:{}", script.display(), column + 1)),
            "{stderr}"
        );
        let lines = stderr.lines().collect::<Vec<_>>();
        let snippet = lines
            .iter()
            .position(|text| text.ends_with(&format!("{line} | {code}")))
            .unwrap_or_else(|| panic!("no script line {line}\n{stderr}"));
        let caret = lines[snippet + 1];
        let code_start = lines[snippet].len() - code.len();
        assert_eq!(caret.find('^'), Some(code_start + column), "{stderr}");
    }
    assert!(stderr.contains("1 | struct P:"), "{stderr}");
    assert!(!stderr.contains("p.nope(), v.len()"), "{stderr}");
    assert!(!stderr.contains("String::nope"), "{stderr}");
    let _ = fs::remove_dir_all(dir);
}
