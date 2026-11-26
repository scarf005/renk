#[cfg(target_os = "linux")]
mod server;

#[cfg(target_os = "linux")]
fn main() -> std::io::Result<()> {
    server::main()
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("renk-web runs on Linux only");
    std::process::ExitCode::FAILURE
}
