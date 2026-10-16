use std::{env, fs, path::PathBuf, process::Command};

fn temp_dir(name: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("rusk-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn script_runs_through_cargo_script_and_forwards_args_and_exit_code() {
    let dir = temp_dir("script-runner");
    let script = dir.join("echo-args.rsk");
    fs::write(
        &script,
        "#!/usr/bin/env -S rusk script\n---\n[package]\nedition = \"2024\"\n---\n\nfn main():\n    let args = std::env::args().skip(1).collect[Vec[String]]()\n    println!(\"{}\", args.join(\"|\"))\n    std::process::exit(3)\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rusk"))
        .args(["script", script.to_str().unwrap(), "a b", "--flag", "-x"])
        .env("XDG_CACHE_HOME", dir.join("cache"))
        .output()
        .unwrap();

    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "a b|--flag|-x\n",
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
    fs::write(&script, "#!/usr/bin/env -S rusk script\nfn main():\n\tprintln!()\n").unwrap();

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
