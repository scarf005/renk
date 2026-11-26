use std::{
    collections::BTreeMap,
    env, fs,
    io::Read,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::process::{CommandExt, ExitStatusExt};

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{ConnectInfo, DefaultBodyLimit, State},
    http::{
        HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri,
        header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HOST, ORIGIN},
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::{Value, json};
use tokio::signal::unix::{SignalKind, signal};

const BYTES_PER_KIB: usize = 1024;
const BYTES_PER_MIB: u64 = 1024 * 1024;
const MAX_RUN_SOURCE_KIB: usize = 64;
const MAX_OUTPUT_KIB: usize = 256;
const JSON_BODY_SIZE_MULTIPLIER: usize = 2;
const MAX_RUN_SOURCE_BYTES: usize = MAX_RUN_SOURCE_KIB * BYTES_PER_KIB;
const MAX_OUTPUT_BYTES: usize = MAX_OUTPUT_KIB * BYTES_PER_KIB;
const MAX_BODY_BYTES: usize = MAX_RUN_SOURCE_BYTES * JSON_BODY_SIZE_MULTIPLIER;
const DEFAULT_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_DIST: &str = "/app/web/dist";
const DEFAULT_RUN_DEPS: &str = "/app/runner/target/release/deps";
const DEFAULT_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_MAX_REQUESTS: usize = 64;
const DEFAULT_MAX_CONCURRENT_RUNS: usize = 2;
const DEFAULT_RUN_RATE_LIMIT_PER_MINUTE: usize = 10;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
/// When the rate table holds this many clients with a run in the last minute, a new client
/// gets 429.
const MAX_RATE_LIMITED_CLIENTS: usize = 4096;
/// rustc writes the program, about 5.5 MB for the trpl example.
const COMPILE_FILE_SIZE_MIB: u64 = 64;
const CHILD_OPEN_FILES: u64 = 64;
const COMPILE_FILE_SIZE_BYTES: u64 = COMPILE_FILE_SIZE_MIB * BYTES_PER_MIB;
/// One byte over the output limit, so the server can see that the output was cut.
const RUN_FILE_SIZE_BYTES: u64 = MAX_OUTPUT_BYTES as u64 + 1;
/// rustc finds its toolchain through these. The submitted program gets the same variables.
const CHILD_ENV: [&str; 4] = ["PATH", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "LANG"];

static ACTIVE_REQUESTS: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_RUNS: AtomicUsize = AtomicUsize::new(0);
static RUN_RATES: RunRates = Mutex::new(BTreeMap::new());
/// Process IDs of the steps that `run_command` runs now. Every other child of the server is a
/// process that a step left behind.
static ACTIVE_CHILDREN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

#[derive(Clone, Debug)]
struct AppState {
    config: ServerConfig,
    dist: PathBuf,
    run_deps: PathBuf,
    /// `Host` header values that `/api/run` accepts.
    hosts: Vec<String>,
    /// The number of runs in progress. Each test server has its own counter.
    active_runs: &'static AtomicUsize,
    run_rates: &'static RunRates,
}

/// The start of the current one-minute window and the runs in it, per client.
type RunRates = Mutex<BTreeMap<IpAddr, (Instant, usize)>>;

#[derive(Clone, Copy, Debug)]
struct ServerConfig {
    request_timeout: Duration,
    run_timeout: Duration,
    max_requests: usize,
    max_concurrent_runs: usize,
    run_rate_limit_per_minute: usize,
    sandbox: Sandbox,
}

/// How a compile or run step is kept apart from the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sandbox {
    /// New user, network, IPC and UTS namespaces. The step fails when one of them cannot be made.
    Namespaces,
    /// No namespaces. `RENK_WEB_UNSANDBOXED=1` selects this.
    Unsandboxed,
}

#[derive(Clone, Copy, Debug)]
struct ChildOptions {
    timeout: Duration,
    sandbox: Sandbox,
    /// The size limit of each file that the step writes. A write past it ends the step with
    /// `SIGXFSZ`.
    max_file_bytes: u64,
}

#[derive(Debug)]
struct CounterPermit(&'static AtomicUsize);

impl Drop for CounterPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug)]
struct CommandResult {
    status: Option<i32>,
    stdout: String,
    stderr: String,
    timed_out: bool,
    stdout_truncated: bool,
    stderr_truncated: bool,
    /// The step wrote past `max_file_bytes` and `SIGXFSZ` ended it.
    output_limited: bool,
    elapsed_ms: f64,
}

#[derive(Debug)]
struct RunResponse {
    result: CommandResult,
    ok: bool,
    stage: &'static str,
    compile_ms: f64,
    run_ms: f64,
    total_ms: f64,
}

#[tokio::main]
pub async fn main() -> std::io::Result<()> {
    let addr = env::var("RENK_WEB_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let run_deps = run_dependency_dir();
    require_dependency_rlib(&run_deps, "trpl")?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let state = AppState {
        config: ServerConfig {
            request_timeout: Duration::from_millis(read_u64_env(
                "RENK_REQUEST_TIMEOUT_MS",
                DEFAULT_REQUEST_TIMEOUT_MS,
            )),
            run_timeout: Duration::from_millis(read_u64_env(
                "RENK_RUN_TIMEOUT_MS",
                DEFAULT_TIMEOUT_MS,
            )),
            max_requests: read_usize_env("RENK_MAX_REQUESTS", DEFAULT_MAX_REQUESTS),
            max_concurrent_runs: read_usize_env(
                "RENK_MAX_CONCURRENT_RUNS",
                DEFAULT_MAX_CONCURRENT_RUNS,
            ),
            run_rate_limit_per_minute: read_usize_env(
                "RENK_RUN_RATE_LIMIT_PER_MINUTE",
                DEFAULT_RUN_RATE_LIMIT_PER_MINUTE,
            ),
            sandbox: if env::var_os("RENK_WEB_UNSANDBOXED").is_some_and(|value| value == "1") {
                Sandbox::Unsandboxed
            } else {
                Sandbox::Namespaces
            },
        },
        dist: PathBuf::from(env::var("RENK_WEB_DIST").unwrap_or_else(|_| DEFAULT_DIST.to_string())),
        run_deps,
        hosts: allowed_hosts(
            listener.local_addr()?,
            &env::var("RENK_WEB_HOSTS").unwrap_or_default(),
        ),
        active_runs: &ACTIVE_RUNS,
        run_rates: &RUN_RATES,
    };
    become_child_subreaper();
    remove_stale_run_dirs(&env::temp_dir(), state.config.run_timeout);
    log_namespace_support(state.config.sandbox)?;
    eprintln!("renk-web listening on http://{addr}");
    let app = router(state).into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal()?)
        .await
        .map_err(std::io::Error::other)
}

fn log_namespace_support(sandbox: Sandbox) -> std::io::Result<()> {
    let dir = temp_run_dir();
    fs::create_dir_all(&dir)?;
    let options = ChildOptions {
        timeout: Duration::from_secs(5),
        sandbox: Sandbox::Namespaces,
        max_file_bytes: COMPILE_FILE_SIZE_BYTES,
    };
    let probe = run_command("rustc", &["--version"], &dir, options);
    let _ = fs::remove_dir_all(dir);
    let mode = match sandbox {
        Sandbox::Namespaces => "runs need them",
        Sandbox::Unsandboxed => "RENK_WEB_UNSANDBOXED=1 runs code without them",
    };
    if probe.status == Some(0) {
        eprintln!("renk-web: namespaces work, {mode}");
    } else {
        eprintln!(
            "renk-web: namespaces do not work ({}), {mode}",
            probe.stderr.trim()
        );
    }
    Ok(())
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/run", post(handle_run_request))
        .fallback(handle_static_request)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            limit_active_requests,
        ))
        .layer(middleware::map_response(add_security_headers))
        .with_state(state)
}

/// Ends on ctrl-c or SIGTERM. SIGTERM counts from the call on, before the server polls the
/// future.
fn shutdown_signal() -> std::io::Result<impl Future<Output = ()>> {
    let mut terminate = signal(SignalKind::terminate())?;
    Ok(async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    })
}

/// Removes the run directories that a server left when it died during a run. A step creates its
/// output files in the directory when it starts, so a directory of a live run was modified
/// within the run timeout.
fn remove_stale_run_dirs(dir: &Path, max_age: Duration) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let run_dir = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("renk-run-"))
            && entry.file_type().is_ok_and(|kind| kind.is_dir());
        let age = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok());
        if run_dir && age.is_some_and(|age| age > max_age) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn read_u64_env(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn read_usize_env(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn try_acquire(counter: &'static AtomicUsize, max: usize) -> Option<CounterPermit> {
    loop {
        let current = counter.load(Ordering::Acquire);
        if current >= max {
            return None;
        }
        if counter
            .compare_exchange(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return Some(CounterPermit(counter));
        }
    }
}

async fn limit_active_requests(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let Some(_permit) = try_acquire(&ACTIVE_REQUESTS, state.config.max_requests) else {
        return json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "error": "too many requests" }),
        );
    };
    tokio::time::timeout(state.config.request_timeout, next.run(request))
        .await
        .unwrap_or_else(|_| {
            json_response(
                StatusCode::REQUEST_TIMEOUT,
                json!({ "error": "request timed out" }),
            )
        })
}

/// Accept `/api/run` only as JSON from a page of this server. A page on any site can send a
/// `text/plain` POST here. After DNS rebinding, the browser sends the attacker's domain as `Host`.
fn is_same_site_json(headers: &HeaderMap, hosts: &[String]) -> bool {
    let json = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"));
    let Some(host) = headers.get(HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let origin_matches = headers.get(ORIGIN).is_none_or(|origin| {
        origin
            .to_str()
            .ok()
            .and_then(|origin| origin.split_once("://"))
            .is_some_and(|(_, authority)| authority.eq_ignore_ascii_case(host))
    });
    json && origin_matches
        && hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host))
}

fn allowed_hosts(listen: SocketAddr, extra: &str) -> Vec<String> {
    let port = listen.port();
    [
        listen.to_string(),
        format!("localhost:{port}"),
        format!("127.0.0.1:{port}"),
        format!("[::1]:{port}"),
    ]
    .into_iter()
    .chain(
        extra
            .split(',')
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .map(str::to_string),
    )
    .collect()
}

async fn handle_run_request(
    State(state): State<AppState>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !is_same_site_json(&headers, &state.hosts) {
        return json_response(StatusCode::FORBIDDEN, json!({ "error": "forbidden" }));
    }
    let Ok(value) = serde_json::from_slice::<Value>(&body) else {
        return json_response(StatusCode::BAD_REQUEST, json!({ "error": "invalid json" }));
    };
    let Some(rust) = value.get("rust").and_then(Value::as_str) else {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({ "error": "rust must be a string" }),
        );
    };
    if rust.len() > MAX_RUN_SOURCE_BYTES {
        return json_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            json!({ "error": format!("rust source exceeds {MAX_RUN_SOURCE_KIB} KiB run limit") }),
        );
    }
    let max_runs = state.config.run_rate_limit_per_minute;
    if !try_run_rate_limit(state.run_rates, client.ip(), max_runs) {
        return json_response(
            StatusCode::TOO_MANY_REQUESTS,
            json!({ "error": "run rate limit exceeded" }),
        );
    }
    let Some(permit) = try_acquire(state.active_runs, state.config.max_concurrent_runs) else {
        return json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "error": "too many active runs" }),
        );
    };

    let rust = rust.to_string();
    let options = ChildOptions {
        timeout: state.config.run_timeout,
        sandbox: state.config.sandbox,
        max_file_bytes: RUN_FILE_SIZE_BYTES,
    };
    let run_deps = state.run_deps.clone();
    // Axum drops the handler when the client goes away or the request times out. The job keeps
    // the permit until its programs are gone.
    let job = move || {
        let _permit = permit;
        compile_and_run_rust(&rust, options, &run_deps)
    };
    match tokio::task::spawn_blocking(job).await {
        Ok(Ok(response)) => json_response(StatusCode::OK, run_response_json(response)),
        Ok(Err(error)) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": error.to_string() }),
        ),
        Err(error) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": error.to_string() }),
        ),
    }
}

fn try_run_rate_limit(rates: &RunRates, client: IpAddr, max: usize) -> bool {
    let client = rate_limit_client(client);
    let now = Instant::now();
    let mut rates = rates.lock().unwrap_or_else(PoisonError::into_inner);
    if rates.len() >= MAX_RATE_LIMITED_CLIENTS && !rates.contains_key(&client) {
        rates.retain(|_, (started, _)| now.duration_since(*started) < RATE_LIMIT_WINDOW);
        if rates.len() >= MAX_RATE_LIMITED_CLIENTS {
            return false;
        }
    }
    let (started, runs) = rates.entry(client).or_insert((now, 0));
    if now.duration_since(*started) >= RATE_LIMIT_WINDOW {
        (*started, *runs) = (now, 0);
    }
    if *runs >= max {
        return false;
    }
    *runs += 1;
    true
}

/// One IPv6 client usually gets a whole /64 network, so the rate limit counts the network.
fn rate_limit_client(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(address) => match address.to_ipv4_mapped() {
            Some(address) => IpAddr::V4(address),
            None => IpAddr::V6(Ipv6Addr::from_bits(
                address.to_bits() & !u128::from(u64::MAX),
            )),
        },
        address => address,
    }
}

fn compile_and_run_rust(
    rust: &str,
    options: ChildOptions,
    run_deps: &Path,
) -> std::io::Result<RunResponse> {
    let dir = temp_run_dir();
    fs::create_dir_all(&dir)?;
    let started = Instant::now();
    let source_path = dir.join("main.rs");
    let binary_path = dir.join("main");
    fs::write(&source_path, rust)?;

    let mut compile_args = vec![
        "--edition=2024".to_string(),
        source_path.to_string_lossy().into_owned(),
        "-o".to_string(),
        binary_path.to_string_lossy().into_owned(),
    ];
    let trpl = require_dependency_rlib(run_deps, "trpl")?;
    compile_args.extend([
        "-L".to_string(),
        format!("dependency={}", run_deps.display()),
        "--extern".to_string(),
        format!("trpl={}", trpl.display()),
    ]);
    for native in native_dependency_dirs(run_deps) {
        compile_args.extend(["-L".to_string(), format!("native={}", native.display())]);
    }
    let compile_args = compile_args.iter().map(String::as_str).collect::<Vec<_>>();
    let compile_options = ChildOptions {
        max_file_bytes: COMPILE_FILE_SIZE_BYTES,
        ..options
    };
    let compile = run_command("rustc", &compile_args, &dir, compile_options);
    if compile.status != Some(0) || compile.timed_out {
        let total_ms = elapsed_ms(started);
        let compile_ms = compile.elapsed_ms;
        let _ = fs::remove_dir_all(dir);
        return Ok(RunResponse {
            result: compile,
            ok: false,
            stage: "compile",
            compile_ms,
            run_ms: 0.0,
            total_ms,
        });
    }

    let run = run_command(binary_path.to_str().unwrap(), &[], &dir, options);
    let total_ms = elapsed_ms(started);
    let compile_ms = compile.elapsed_ms;
    let run_ms = run.elapsed_ms;
    let ok = run.status == Some(0) && !run.timed_out;
    let _ = fs::remove_dir_all(dir);
    Ok(RunResponse {
        result: run,
        ok,
        stage: "run",
        compile_ms,
        run_ms,
        total_ms,
    })
}

fn run_dependency_dir() -> PathBuf {
    if let Ok(path) = env::var("RENK_RUN_DEPS") {
        return PathBuf::from(path);
    }
    if let Ok(executable) = env::current_exe()
        && let Some(parent) = executable.parent()
    {
        let local = parent.join("deps");
        if local.is_dir() {
            return local;
        }
    }
    PathBuf::from(DEFAULT_RUN_DEPS)
}

fn find_dependency_rlib(directory: &Path, crate_name: &str) -> Option<PathBuf> {
    let prefix = format!("lib{crate_name}-");
    fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".rlib"))
        })
}

fn require_dependency_rlib(directory: &Path, crate_name: &str) -> std::io::Result<PathBuf> {
    find_dependency_rlib(directory, crate_name).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "runner dependency `{crate_name}` not found in {}",
                directory.display()
            ),
        )
    })
}

fn native_dependency_dirs(run_deps: &Path) -> Vec<PathBuf> {
    let Some(release) = run_deps.parent() else {
        return Vec::new();
    };
    let build = release.join("build");
    let Ok(entries) = fs::read_dir(build) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("out"))
        .filter(|path| path.is_dir())
        .collect()
}

fn temp_run_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    env::temp_dir().join(format!("renk-run-{}-{nanos}", std::process::id()))
}

fn run_command(command: &str, args: &[&str], cwd: &Path, options: ChildOptions) -> CommandResult {
    let timeout = options.timeout;
    let started = Instant::now();
    let stdout_path = temp_output_path(cwd, "stdout");
    let stderr_path = temp_output_path(cwd, "stderr");
    let stdout_file = fs::File::create(&stdout_path);
    let stderr_file = fs::File::create(&stderr_path);
    let (Ok(stdout_file), Ok(stderr_file)) = (stdout_file, stderr_file) else {
        return CommandResult {
            status: None,
            stdout: String::new(),
            stderr: "failed to create command output files".to_string(),
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
            output_limited: false,
            elapsed_ms: elapsed_ms(started),
        };
    };

    let mut command = Command::new(command);
    command
        .env_clear()
        .envs(
            CHILD_ENV
                .iter()
                .filter_map(|name| Some((name, env::var_os(name)?))),
        )
        .env("TMPDIR", cwd)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    configure_child_process(&mut command, options);
    let child = {
        // Keep the lock until the new step is in the list, or `kill_orphans` can kill it.
        let mut active = ACTIVE_CHILDREN
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let child = command.spawn();
        if let Ok(child) = &child {
            active.push(child.id());
        }
        child
    };
    let Ok(mut child) = child else {
        // A failed sandbox step writes its name to the stderr file before `spawn` fails.
        let (step, _) = read_limited_utf8(&stderr_path, MAX_OUTPUT_BYTES);
        let _ = fs::remove_file(stdout_path);
        let _ = fs::remove_file(stderr_path);
        return CommandResult {
            status: None,
            stdout: String::new(),
            stderr: format!("{step}{}", child.unwrap_err()),
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
            output_limited: false,
            elapsed_ms: elapsed_ms(started),
        };
    };
    let child_pid = child.id();

    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() >= timeout => {
                timed_out = true;
                kill_process_tree(child_pid);
                let _ = child.kill();
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                kill_orphans(child_pid);
                let _ = fs::remove_file(stdout_path);
                let _ = fs::remove_file(stderr_path);
                return CommandResult {
                    status: None,
                    stdout: String::new(),
                    stderr: error.to_string(),
                    timed_out,
                    stdout_truncated: false,
                    stderr_truncated: false,
                    output_limited: false,
                    elapsed_ms: elapsed_ms(started),
                };
            }
        }
    }

    let _ = unsafe { libc::kill(-(child_pid as i32), libc::SIGKILL) };
    let status = child.wait();
    kill_orphans(child_pid);
    let (stdout, stdout_truncated) = read_limited_utf8(&stdout_path, MAX_OUTPUT_BYTES);
    let (mut stderr, stderr_truncated) = read_limited_utf8(&stderr_path, MAX_OUTPUT_BYTES);
    let _ = fs::remove_file(stdout_path);
    let _ = fs::remove_file(stderr_path);
    match status {
        Ok(status) => CommandResult {
            status: status.code(),
            stdout,
            stderr,
            timed_out,
            stdout_truncated,
            stderr_truncated,
            output_limited: status.signal() == Some(libc::SIGXFSZ),
            elapsed_ms: elapsed_ms(started),
        },
        Err(error) => {
            if !stderr.is_empty() {
                stderr.push('\n');
            }
            stderr.push_str(&error.to_string());
            CommandResult {
                status: None,
                stdout,
                stderr,
                timed_out,
                stdout_truncated,
                stderr_truncated,
                output_limited: false,
                elapsed_ms: elapsed_ms(started),
            }
        }
    }
}

fn temp_output_path(cwd: &Path, name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    cwd.join(format!("{name}-{}-{nanos}.txt", std::process::id()))
}

fn read_limited_utf8(path: &Path, limit: usize) -> (String, bool) {
    let Ok(mut file) = fs::File::open(path) else {
        return (String::new(), false);
    };
    let mut buffer = vec![0; limit.saturating_add(1)];
    let bytes_read = file.read(&mut buffer).unwrap_or_default();
    let truncated = bytes_read > limit;
    buffer.truncate(bytes_read.min(limit));
    (String::from_utf8_lossy(&buffer).to_string(), truncated)
}

#[cfg(target_os = "linux")]
fn configure_child_process(command: &mut Command, options: ChildOptions) {
    let cpu_seconds = options.timeout.as_secs().saturating_add(1).max(1);
    // Format the ID maps here: the code between `fork` and `exec` must not allocate.
    let id_maps = (options.sandbox == Sandbox::Namespaces).then(|| {
        let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
        (format!("{uid} {uid} 1"), format!("{gid} {gid} 1"))
    });
    unsafe {
        command.pre_exec(move || {
            create_child_process_group()?;
            sandbox_step(
                "no_new_privs",
                libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) == 0,
            )?;
            // Set the limits before `enter_namespaces` forks, so both processes get them.
            apply_child_limits(cpu_seconds, options.max_file_bytes)?;
            match &id_maps {
                Some((uid_map, gid_map)) => enter_namespaces(uid_map, gid_map),
                None => Ok(()),
            }
        });
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_child_process(_command: &mut Command, _options: ChildOptions) {}

#[cfg(unix)]
fn create_child_process_group() -> std::io::Result<()> {
    if unsafe { libc::setpgid(0, 0) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Runs between `fork` and `exec`, so it makes only async-signal-safe calls.
#[cfg(target_os = "linux")]
fn enter_namespaces(uid_map: &str, gid_map: &str) -> std::io::Result<()> {
    let flags = libc::CLONE_NEWUSER
        | libc::CLONE_NEWNET
        | libc::CLONE_NEWIPC
        | libc::CLONE_NEWUTS
        | libc::CLONE_NEWPID;
    sandbox_step("unshare", unsafe { libc::unshare(flags) } == 0)?;
    write_proc_file("setgroups", c"/proc/self/setgroups", b"deny")?;
    write_proc_file("uid_map", c"/proc/self/uid_map", uid_map.as_bytes())?;
    write_proc_file("gid_map", c"/proc/self/gid_map", gid_map.as_bytes())?;
    fork_pid_one()
}

/// Forks PID 1 of the new PID namespace, which goes on to `exec` the step. This process stays
/// outside the namespace, waits for PID 1 and exits with its status. When the server kills this
/// process, `PR_SET_PDEATHSIG` kills PID 1, and the kernel then kills the whole namespace.
#[cfg(target_os = "linux")]
fn fork_pid_one() -> std::io::Result<()> {
    let parent = unsafe { libc::syscall(libc::SYS_pidfd_open, libc::getpid(), 0) } as libc::c_int;
    sandbox_step("pidfd_open", parent >= 0)?;
    let pid = unsafe { libc::fork() };
    sandbox_step("fork", pid >= 0)?;
    if pid == 0 {
        let pdeathsig = unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) };
        sandbox_step("pdeathsig", pdeathsig == 0)?;
        // The parent can exit before `PR_SET_PDEATHSIG`. `getppid` returns 0 for a parent in
        // another PID namespace, so ask the pidfd, which turns readable when the parent exits.
        let mut poll = libc::pollfd {
            fd: parent,
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut poll, 1, 0) } != 0 {
            unsafe { libc::_exit(1) };
        }
        unsafe { libc::close(parent) };
        return Ok(());
    }
    // `spawn` waits until every copy of its `exec` error pipe is closed. Only PID 1 calls
    // `exec`, so close the copy of this process.
    unsafe { libc::syscall(libc::SYS_close_range, 3, libc::c_uint::MAX, 0) };
    let mut status = 0;
    while unsafe { libc::waitpid(pid, &mut status, 0) } == -1
        && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
    {}
    unsafe {
        if libc::WIFSIGNALED(status) {
            let signal = libc::WTERMSIG(status);
            libc::signal(signal, libc::SIG_DFL);
            libc::kill(libc::getpid(), signal);
            libc::_exit(128 + signal);
        }
        libc::_exit(libc::WEXITSTATUS(status))
    }
}

#[cfg(target_os = "linux")]
fn write_proc_file(step: &str, path: &std::ffi::CStr, contents: &[u8]) -> std::io::Result<()> {
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC) };
    sandbox_step(step, fd >= 0)?;
    let written = unsafe { libc::write(fd, contents.as_ptr().cast(), contents.len()) };
    let result = sandbox_step(step, written == contents.len() as isize);
    unsafe { libc::close(fd) };
    result
}

/// Returns `errno` when a step failed, and writes the step name to stderr for `run_command` to
/// show. `spawn` returns only the error number.
#[cfg(target_os = "linux")]
fn sandbox_step(step: &str, succeeded: bool) -> std::io::Result<()> {
    if succeeded {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    for part in ["sandbox step `", step, "` failed: "] {
        unsafe { libc::write(libc::STDERR_FILENO, part.as_ptr().cast(), part.len()) };
    }
    Err(error)
}

#[cfg(unix)]
fn apply_child_limits(cpu_seconds: u64, max_file_bytes: u64) -> std::io::Result<()> {
    set_limit(libc::RLIMIT_CPU, cpu_seconds, cpu_seconds)?;
    set_limit(libc::RLIMIT_CORE, 0, 0)?;
    set_limit(libc::RLIMIT_FSIZE, max_file_bytes, max_file_bytes)?;
    set_limit(libc::RLIMIT_NOFILE, CHILD_OPEN_FILES, CHILD_OPEN_FILES)
}

#[cfg(unix)]
fn set_limit(resource: libc::__rlimit_resource_t, soft: u64, hard: u64) -> std::io::Result<()> {
    let limit = libc::rlimit {
        rlim_cur: soft as libc::rlim_t,
        rlim_max: hard as libc::rlim_t,
    };
    if unsafe { libc::setrlimit(resource, &limit) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Marks the server as a child subreaper: the kernel makes it the parent of every process that
/// a step leaves behind, so `kill_orphans` can find them.
fn become_child_subreaper() {
    let _ = unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) };
}

/// Removes `finished` from the active steps, then kills and reaps every other child of the
/// server with its descendants, until none is left.
fn kill_orphans(finished: u32) {
    let mut active = ACTIVE_CHILDREN
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    active.retain(|pid| *pid != finished);
    loop {
        let orphans = child_processes(std::process::id())
            .into_iter()
            .filter(|pid| !active.contains(pid))
            .collect::<Vec<_>>();
        if orphans.is_empty() {
            return;
        }
        for pid in orphans {
            kill_process_tree(pid);
            let _ = unsafe { libc::waitpid(pid as i32, std::ptr::null_mut(), 0) };
        }
    }
}

/// Call only for a process that is not reaped yet, or its ID can belong to another process.
/// `SIGSTOP` comes first, so the process cannot fork while its children are read.
fn kill_process_tree(pid: u32) {
    let _ = unsafe { libc::kill(pid as i32, libc::SIGSTOP) };
    for child in child_processes(pid) {
        kill_process_tree(child);
    }
    let _ = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    let _ = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
}

fn child_processes(parent: u32) -> Vec<u32> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            let pid = entry.ok()?.file_name().to_str()?.parse::<u32>().ok()?;
            let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            // The command name in parentheses can hold spaces, so count fields after the last `)`.
            let ppid = stat.rsplit_once(')')?.1.split_whitespace().nth(1)?;
            (ppid.parse() == Ok(parent)).then_some(pid)
        })
        .collect()
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn run_response_json(response: RunResponse) -> Value {
    json!({
        "ok": response.ok,
        "stage": response.stage,
        "status": response.result.status,
        "stdout": response.result.stdout,
        "stderr": response.result.stderr,
        "timedOut": response.result.timed_out,
        "stdoutTruncated": response.result.stdout_truncated,
        "stderrTruncated": response.result.stderr_truncated,
        "outputLimited": response.result.output_limited,
        "compileMs": response.compile_ms,
        "runMs": response.run_ms,
        "totalMs": response.total_ms,
    })
}

async fn handle_static_request(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return json_response(
            StatusCode::METHOD_NOT_ALLOWED,
            json!({ "error": "method not allowed" }),
        );
    }
    serve_static(&state.dist, uri.path(), method == Method::HEAD)
}

fn serve_static(dist: &Path, path: &str, head_only: bool) -> Response {
    let relative = route_path(path);
    let file = safe_join(dist, &relative)
        .filter(|path| path.is_file())
        .or_else(|| {
            if is_app_route(path) {
                Some(dist.join("index.html"))
            } else {
                None
            }
        });
    let Some(file) = file else {
        return text_response(StatusCode::NOT_FOUND, "not found", head_only);
    };
    match fs::read(&file) {
        Ok(body) => body_response(StatusCode::OK, content_type(&file), body, head_only),
        Err(error) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": error.to_string() }),
        ),
    }
}

fn route_path(path: &str) -> String {
    if path == "/" {
        "index.html".to_string()
    } else {
        path.trim_start_matches('/').to_string()
    }
}

fn is_app_route(path: &str) -> bool {
    matches!(path, "/") || path.starts_with("/examples/")
}

fn safe_join(root: &Path, relative: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => path.push(part),
            _ => return None,
        }
    }
    Some(path)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

fn json_response(status: StatusCode, value: Value) -> Response {
    (status, Json(value)).into_response()
}

fn text_response(status: StatusCode, body: &'static str, head_only: bool) -> Response {
    body_response(
        status,
        "text/plain; charset=utf-8",
        body.as_bytes().to_vec(),
        head_only,
    )
}

fn body_response(
    status: StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
    head_only: bool,
) -> Response {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, content_type)
        .header(CONTENT_LENGTH, body.len().to_string())
        .body(if head_only {
            Body::empty()
        } else {
            Body::from(body)
        })
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn add_security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    insert_static_header(
        headers,
        "content-security-policy",
        "default-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'",
    );
    insert_static_header(headers, "cross-origin-embedder-policy", "require-corp");
    insert_static_header(headers, "cross-origin-opener-policy", "same-origin");
    insert_static_header(headers, "cross-origin-resource-policy", "same-origin");
    insert_static_header(
        headers,
        "permissions-policy",
        "camera=(), microphone=(), geolocation=(), payment=(), usb=(), serial=(), bluetooth=(), interest-cohort=()",
    );
    insert_static_header(headers, "referrer-policy", "no-referrer");
    insert_static_header(
        headers,
        "strict-transport-security",
        "max-age=31536000; includeSubDomains; preload",
    );
    insert_static_header(headers, "x-content-type-options", "nosniff");
    insert_static_header(headers, "x-frame-options", "DENY");
    response
}

fn insert_static_header(headers: &mut HeaderMap, name: &'static str, value: &'static str) {
    headers.insert(
        HeaderName::from_static(name),
        HeaderValue::from_static(value),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, extract::ConnectInfo, http::Request};
    use tower::ServiceExt;

    fn temp_dir(name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!("renk-web-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn state(dist: &Path, run_rate_limit_per_minute: usize) -> AppState {
        AppState {
            config: ServerConfig {
                request_timeout: Duration::from_secs(5),
                run_timeout: Duration::from_secs(1),
                max_requests: 8,
                max_concurrent_runs: 1,
                run_rate_limit_per_minute,
                sandbox: Sandbox::Unsandboxed,
            },
            dist: dist.to_path_buf(),
            run_deps: dist.join("missing-deps"),
            hosts: allowed_hosts(
                "0.0.0.0:8080".parse().unwrap(),
                "renk.example.com, Other.example:8443",
            ),
            active_runs: Box::leak(Box::default()),
            run_rates: Box::leak(Box::default()),
        }
    }

    fn unsandboxed(timeout: Duration) -> ChildOptions {
        ChildOptions {
            timeout,
            sandbox: Sandbox::Unsandboxed,
            max_file_bytes: RUN_FILE_SIZE_BYTES,
        }
    }

    /// A server with namespaces on. Its `trpl` is an empty file in the target directory.
    fn namespace_state(run_timeout: Duration) -> AppState {
        let exe = env::current_exe().unwrap();
        let deps = exe.ancestors().nth(2).unwrap().join("renk-web-namespaces");
        fs::create_dir_all(&deps).unwrap();
        fs::write(deps.join("libtrpl-0.rlib"), "").unwrap();
        let mut state = state(&deps, usize::MAX);
        state.run_deps = deps;
        state.config.sandbox = Sandbox::Namespaces;
        state.config.run_timeout = run_timeout;
        state.config.request_timeout = run_timeout * 3;
        state
    }

    /// Sends `rust` to `/api/run` of a server with namespaces on and a run timeout of 2 s.
    async fn run_in_namespaces(rust: &str) -> Value {
        let app = router(namespace_state(Duration::from_secs(2)));
        let (status, _, body) = send(&app, run(json!({ "rust": rust }).to_string())).await;
        let body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn send(app: &Router, request: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, headers, body.to_vec())
    }

    fn get(method: Method, path: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .unwrap()
    }

    fn run_with_headers(headers: &[(&str, &str)], body: impl Into<Body>) -> Request<Body> {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/api/run")
            .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        request.body(body.into()).unwrap()
    }

    fn run(body: impl Into<Body>) -> Request<Body> {
        run_with_headers(
            &[
                ("host", "localhost:8080"),
                ("content-type", "application/json"),
            ],
            body,
        )
    }

    #[tokio::test]
    async fn run_requests_need_json_and_a_host_and_origin_of_this_server() {
        let dist = temp_dir("origin");
        let app = router(state(&dist, 100));
        let json = ("content-type", "application/json");
        let local = ("host", "localhost:8080");
        let cases: &[(&[(&str, &str)], StatusCode)] = &[
            (&[json, local], StatusCode::BAD_REQUEST),
            (&[json, ("host", "0.0.0.0:8080")], StatusCode::BAD_REQUEST),
            (&[json, ("host", "[::1]:8080")], StatusCode::BAD_REQUEST),
            (
                &[
                    ("content-type", "Application/JSON; charset=utf-8"),
                    ("host", "127.0.0.1:8080"),
                    ("origin", "http://127.0.0.1:8080"),
                ],
                StatusCode::BAD_REQUEST,
            ),
            (
                &[
                    json,
                    ("host", "renk.example.com"),
                    ("origin", "https://renk.example.com"),
                ],
                StatusCode::BAD_REQUEST,
            ),
            (
                &[json, ("host", "other.example:8443")],
                StatusCode::BAD_REQUEST,
            ),
            (
                &[("content-type", "text/plain"), local],
                StatusCode::FORBIDDEN,
            ),
            (
                &[("content-type", "application/jsonx"), local],
                StatusCode::FORBIDDEN,
            ),
            (&[local], StatusCode::FORBIDDEN),
            (&[json], StatusCode::FORBIDDEN),
            (&[json, ("host", "localhost:9999")], StatusCode::FORBIDDEN),
            (
                &[json, ("host", "evil.example:8080")],
                StatusCode::FORBIDDEN,
            ),
            (&[json, ("host", "other.example")], StatusCode::FORBIDDEN),
            (
                &[json, local, ("origin", "https://evil.example")],
                StatusCode::FORBIDDEN,
            ),
            (
                &[json, local, ("origin", "http://localhost:9999")],
                StatusCode::FORBIDDEN,
            ),
            (&[json, local, ("origin", "null")], StatusCode::FORBIDDEN),
            (
                &[json, local, ("origin", "http://localhost:8080/x")],
                StatusCode::FORBIDDEN,
            ),
        ];
        for (headers, expected) in cases {
            let request = run_with_headers(headers, r#"{"rust": 1}"#);
            let (status, _, _) = send(&app, request).await;
            assert_eq!(status, *expected, "{headers:?}");
        }
        let _ = fs::remove_dir_all(dist);
    }

    #[test]
    fn safe_join_rejects_every_non_normal_component() {
        let root = Path::new("/srv/dist");
        assert_eq!(
            safe_join(root, "assets/app.js"),
            Some(root.join("assets/app.js"))
        );
        for relative in [
            "../etc/passwd",
            "assets/../../x",
            "/etc/passwd",
            "./index.html",
        ] {
            assert_eq!(safe_join(root, relative), None, "{relative}");
        }
        assert_eq!(safe_join(root, "%2e%2e/x"), Some(root.join("%2e%2e/x")));
    }

    #[test]
    fn counters_release_permits_on_drop() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let first = try_acquire(&COUNTER, 2).unwrap();
        let second = try_acquire(&COUNTER, 2).unwrap();
        assert!(try_acquire(&COUNTER, 2).is_none());
        drop(first);
        let third = try_acquire(&COUNTER, 2).unwrap();
        drop((second, third));
        assert_eq!(COUNTER.load(Ordering::Acquire), 0);
    }

    #[test]
    fn limited_reads_flag_truncation_exactly_past_the_limit() {
        let dir = temp_dir("limited");
        let path = dir.join("out.txt");
        fs::write(&path, "12345").unwrap();
        assert_eq!(read_limited_utf8(&path, 5), ("12345".to_string(), false));
        assert_eq!(read_limited_utf8(&path, 4), ("1234".to_string(), true));
        fs::write(&path, "é").unwrap();
        assert_eq!(read_limited_utf8(&path, 1), ("\u{fffd}".to_string(), true));
        assert_eq!(
            read_limited_utf8(&dir.join("missing"), 4),
            (String::new(), false)
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn commands_report_status_output_and_spawn_failures() {
        let dir = temp_dir("command");
        let result = run_command(
            "sh",
            &["-c", "echo out; echo err >&2; exit 3"],
            &dir,
            unsandboxed(Duration::from_secs(5)),
        );
        assert_eq!(result.status, Some(3));
        assert_eq!(
            (result.stdout.as_str(), result.stderr.as_str()),
            ("out\n", "err\n")
        );
        assert!(!result.timed_out);

        let result = run_command(
            "/nonexistent/renk-binary",
            &[],
            &dir,
            unsandboxed(Duration::from_secs(5)),
        );
        assert_eq!(result.status, None);
        assert!(!result.stderr.is_empty());
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            0,
            "output files must be removed"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// `cargo test` sets `CARGO_*` variables in this process. The child must not see them.
    #[test]
    fn commands_get_only_the_toolchain_environment() {
        let dir = temp_dir("env");
        let result = run_command("env", &[], &dir, unsandboxed(Duration::from_secs(5)));
        let variables = result
            .stdout
            .lines()
            .filter_map(|line| line.split_once('='))
            .collect::<Vec<_>>();
        let names = variables.iter().map(|(name, _)| *name).collect::<Vec<_>>();
        assert!(
            names.iter().all(|name| {
                ["PATH", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "LANG", "TMPDIR"].contains(name)
            }),
            "{names:?}"
        );
        assert!(names.contains(&"PATH"), "{names:?}");
        assert!(variables.contains(&("TMPDIR", dir.to_str().unwrap())));
        let _ = fs::remove_dir_all(dir);
    }

    /// Where user namespaces work, `readlink /proc/self/ns/net` prints another network namespace
    /// than the server's. Where they do not (some containers and sandboxes), the step fails and
    /// names the namespace step. `RENK_WEB_UNSANDBOXED=1` mode runs it in the server's namespace.
    #[cfg(target_os = "linux")]
    #[test]
    fn sandboxed_commands_get_their_own_network_or_fail() {
        let dir = temp_dir("netns");
        let server = fs::read_link("/proc/self/ns/net").unwrap();
        let readlink = |sandbox| {
            let options = ChildOptions {
                sandbox,
                ..unsandboxed(Duration::from_secs(5))
            };
            run_command("readlink", &["/proc/self/ns/net"], &dir, options)
        };
        let sandboxed = readlink(Sandbox::Namespaces);
        if sandboxed.status == Some(0) {
            assert_ne!(sandboxed.stdout.trim(), server.to_str().unwrap());
        } else {
            assert_eq!(sandboxed.status, None);
            assert!(
                sandboxed.stderr.starts_with("sandbox step `"),
                "{}",
                sandboxed.stderr
            );
        }
        let unsandboxed = readlink(Sandbox::Unsandboxed);
        assert_eq!(unsandboxed.stdout.trim(), server.to_str().unwrap());
        let _ = fs::remove_dir_all(dir);
    }

    #[tokio::test]
    #[ignore = "needs unprivileged user namespaces"]
    async fn runs_a_program_in_namespaces() {
        let body = run_in_namespaces(r#"fn main() { println!("hello") }"#).await;
        let result = (&body["ok"], &body["stdout"]);
        assert_eq!(result, (&json!(true), &json!("hello\n")), "{body}");
    }

    /// Makes a directory `renk-run-0-<test pid>` beside the run directories, then lists the
    /// parent of the program's working directory. The program must find only its own directory.
    #[tokio::test]
    #[ignore = "needs unprivileged user namespaces"]
    async fn hides_other_run_directories_from_a_program() {
        let other = env::temp_dir().join(format!("renk-run-0-{}", std::process::id()));
        fs::create_dir_all(&other).unwrap();
        let body = run_in_namespaces(
            r#"fn main() {
                let cwd = std::env::current_dir().unwrap();
                for entry in std::fs::read_dir(cwd.parent().unwrap()).unwrap() {
                    println!("{}", entry.unwrap().path().display());
                }
                println!("{}", cwd.display());
            }"#,
        )
        .await;
        let _ = fs::remove_dir_all(other);
        let stdout = body["stdout"].as_str().unwrap();
        let lines = stdout.lines().collect::<Vec<_>>();
        assert!(lines.len() == 2 && lines[0] == lines[1], "{body}");
    }

    /// Reads `/proc/1/environ` and lists the process directories in `/proc`. The program must
    /// find its own `TMPDIR` there and no process but PID 1, itself.
    #[tokio::test]
    #[ignore = "needs unprivileged user namespaces"]
    async fn shows_only_the_program_in_proc() {
        let body = run_in_namespaces(
            r#"fn main() {
                let environ = std::fs::read("/proc/1/environ").unwrap_or_default();
                let tmpdir = format!("TMPDIR={}", std::env::var("TMPDIR").unwrap());
                let pids = std::fs::read_dir("/proc")
                    .unwrap()
                    .filter_map(|entry| entry.unwrap().file_name().into_string().ok())
                    .filter(|name| name.parse::<u32>().is_ok())
                    .collect::<Vec<_>>();
                let own = String::from_utf8_lossy(&environ).split('\0').any(|var| var == tmpdir);
                println!("{own} {pids:?}");
            }"#,
        )
        .await;
        assert_eq!(body["stdout"], json!("true [\"1\"]\n"), "{body}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn commands_are_killed_with_their_process_group_on_timeout() {
        let dir = temp_dir("timeout");
        let started = Instant::now();
        let result = run_command(
            "sh",
            &["-c", "sleep 30 & echo $! > child.pid; wait"],
            &dir,
            unsandboxed(Duration::from_millis(300)),
        );
        assert!(result.timed_out);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
        let pid = fs::read_to_string(dir.join("child.pid")).unwrap();
        // SIGKILL is delivered asynchronously; a killed child may linger briefly or as a zombie.
        let alive = || {
            fs::read_to_string(format!("/proc/{}/stat", pid.trim()))
                .is_ok_and(|stat| !stat.contains(") Z "))
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        while alive() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!alive(), "background child {pid} survived the timeout");
        let _ = fs::remove_dir_all(dir);
    }

    /// `setsid -f sleep 60.<test pid>` moves the `sleep` out of the process group, so a kill of
    /// the group misses it. The odd argument finds the `sleep` in `/proc`. One second after the
    /// 300 ms timeout it must be gone in both modes. The program also runs `kill -0` on the
    /// server, which must fail in namespace mode.
    #[cfg(target_os = "linux")]
    #[test]
    fn programs_that_leave_the_process_group_are_killed_on_timeout() {
        become_child_subreaper();
        let server = std::process::id();
        let sleep = format!("60.{server}");
        let running = || {
            fs::read_dir("/proc")
                .unwrap()
                .filter_map(|entry| fs::read(entry.ok()?.path().join("cmdline")).ok())
                .any(|cmdline| {
                    cmdline
                        .split(|byte| *byte == 0)
                        .any(|arg| arg == sleep.as_bytes())
                })
        };
        for sandbox in [Sandbox::Namespaces, Sandbox::Unsandboxed] {
            let dir = temp_dir("setsid");
            let options = ChildOptions {
                sandbox,
                ..unsandboxed(Duration::from_millis(300))
            };
            let script =
                format!("setsid -f sleep {sleep}; kill -0 {server} && echo signalled; sleep 60");
            let result = run_command("sh", &["-c", &script], &dir, options);
            let _ = fs::remove_dir_all(dir);
            if result.stderr.starts_with("sandbox step `") {
                eprintln!("namespace mode not tested: {}", result.stderr);
                continue;
            }
            assert!(result.timed_out, "{result:?}");
            assert_eq!(
                result.stdout.contains("signalled"),
                sandbox == Sandbox::Unsandboxed,
                "{sandbox:?}"
            );
            let deadline = Instant::now() + Duration::from_secs(1);
            while running() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            assert!(
                !running(),
                "{sandbox:?}: sleep {sleep} survived the timeout"
            );
        }
    }

    #[test]
    fn command_output_is_capped() {
        let dir = temp_dir("cap");
        let result = run_command(
            "sh",
            &[
                "-c",
                &format!("head -c {} /dev/zero | tr '\\\\0' x", MAX_OUTPUT_BYTES + 10),
            ],
            &dir,
            unsandboxed(Duration::from_secs(5)),
        );
        assert_eq!(result.stdout.len(), MAX_OUTPUT_BYTES);
        assert!(result.stdout_truncated && !result.stderr_truncated);
        let _ = fs::remove_dir_all(dir);
    }

    /// A program can write one byte past the output limit, so the server sees that the output
    /// was cut. One more byte ends the program with `SIGXFSZ`, and the answer says so.
    #[test]
    fn output_past_the_limit_ends_the_program() {
        let dir = temp_dir("output-limit");
        let write = |bytes: usize| {
            let script = format!("exec head -c {bytes} /dev/zero");
            let options = unsandboxed(Duration::from_secs(5));
            run_command("sh", &["-c", &script], &dir, options)
        };
        let fits = write(MAX_OUTPUT_BYTES + 1);
        let fits = (fits.status, fits.stdout_truncated, fits.output_limited);
        assert_eq!(fits, (Some(0), true, false));
        let flood = write(MAX_OUTPUT_BYTES + 2);
        assert_eq!((flood.status, flood.stdout_truncated), (None, true));
        let response = RunResponse {
            result: flood,
            ok: false,
            stage: "run",
            compile_ms: 0.0,
            run_ms: 0.0,
            total_ms: 0.0,
        };
        assert_eq!(run_response_json(response)["outputLimited"], json!(true));
        let _ = fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn sigterm_shuts_the_server_down() {
        let shutdown = shutdown_signal().unwrap();
        unsafe { libc::kill(libc::getpid(), libc::SIGTERM) };
        let stopped = tokio::time::timeout(Duration::from_secs(5), shutdown).await;
        assert!(stopped.is_ok(), "no shutdown 5 s after SIGTERM");
    }

    /// The run timeout is 5 s. Removes `renk-run-1-old`, modified 10 s ago. Keeps a fresh run
    /// directory, an old file with the same prefix, and an old directory with another name.
    #[test]
    fn startup_removes_only_old_run_directories() {
        let dir = temp_dir("stale");
        let old = dir.join("renk-run-1-old");
        let fresh = dir.join("renk-run-1-fresh");
        let file = dir.join("renk-run-1-file");
        let other = dir.join("other");
        for path in [&old, &fresh, &other] {
            fs::create_dir(path).unwrap();
        }
        fs::write(old.join("main.rs"), "").unwrap();
        fs::write(&file, "").unwrap();
        let ten_seconds_ago = SystemTime::now() - Duration::from_secs(10);
        for path in [&old, &file, &other] {
            let path = fs::File::open(path).unwrap();
            path.set_modified(ten_seconds_ago).unwrap();
        }

        remove_stale_run_dirs(&dir, Duration::from_secs(5));
        let exists = [&old, &fresh, &file, &other].map(|path| path.exists());
        assert_eq!(exists, [false, true, true, true]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn default_run_deps_match_the_image() {
        let containerfile = include_str!("../../../../../Containerfile");
        let line = format!("RENK_RUN_DEPS={DEFAULT_RUN_DEPS} ");
        assert!(
            containerfile.contains(&line),
            "no `{line}` in the Containerfile"
        );
    }

    #[tokio::test]
    async fn static_files_are_served_inside_dist_only() {
        let dist = temp_dir("static");
        fs::write(dist.join("index.html"), "<p>app</p>").unwrap();
        fs::create_dir_all(dist.join("assets")).unwrap();
        fs::write(dist.join("assets/app.wasm"), [0, 97, 115, 109]).unwrap();
        fs::write(dist.parent().unwrap().join("renk-web-secret.txt"), "secret").unwrap();
        let app = router(state(&dist, 100));

        let (status, headers, body) = send(&app, get(Method::GET, "/assets/app.wasm")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[CONTENT_TYPE], "application/wasm");
        assert_eq!(body, [0, 97, 115, 109]);

        let (status, _, body) = send(&app, get(Method::GET, "/examples/hello")).await;
        assert_eq!(
            (status, body.as_slice()),
            (StatusCode::OK, b"<p>app</p>".as_slice())
        );

        let (status, headers, body) = send(&app, get(Method::HEAD, "/")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[CONTENT_LENGTH], "10");
        assert!(body.is_empty());

        for path in [
            "/../renk-web-secret.txt",
            "/assets/../../renk-web-secret.txt",
            "/missing.js",
            "/rust-examples/hello",
        ] {
            let (status, _, body) = send(&app, get(Method::GET, path)).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert!(!String::from_utf8_lossy(&body).contains("secret"), "{path}");
        }
        let (status, _, _) = send(&app, get(Method::DELETE, "/index.html")).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        let _ = fs::remove_file(dist.parent().unwrap().join("renk-web-secret.txt"));
        let _ = fs::remove_dir_all(dist);
    }

    #[tokio::test]
    async fn every_response_carries_the_security_headers() {
        let dist = temp_dir("headers");
        let app = router(state(&dist, 100));
        for request in [
            get(Method::GET, "/missing"),
            get(Method::POST, "/index.html"),
        ] {
            let (_, headers, _) = send(&app, request).await;
            for name in [
                "content-security-policy",
                "cross-origin-opener-policy",
                "strict-transport-security",
                "x-content-type-options",
                "x-frame-options",
            ] {
                assert!(headers.contains_key(name), "{name}");
            }
            assert_eq!(headers[CACHE_CONTROL], "no-store");
        }
        let _ = fs::remove_dir_all(dist);
    }

    #[tokio::test]
    async fn run_requests_are_validated() {
        let dist = temp_dir("run");
        let app = router(state(&dist, 5));
        let error = |body: &[u8]| serde_json::from_slice::<Value>(body).unwrap()["error"].clone();

        let (status, _, body) = send(&app, run("not json")).await;
        assert_eq!(
            (status, error(&body)),
            (StatusCode::BAD_REQUEST, json!("invalid json"))
        );
        let (status, _, body) = send(&app, run(r#"{"rust": 1}"#)).await;
        assert_eq!(
            (status, error(&body)),
            (StatusCode::BAD_REQUEST, json!("rust must be a string"))
        );
        let source = "x".repeat(MAX_RUN_SOURCE_BYTES + 1);
        let (status, _, _) = send(&app, run(json!({ "rust": source }).to_string())).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        let (status, _, body) = send(&app, run(r#"{"rust": "fn main() {}"}"#)).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            error(&body)
                .as_str()
                .unwrap()
                .contains("runner dependency `trpl` not found")
        );
        let (status, _, _) = send(&app, run("x".repeat(MAX_BODY_BYTES + 1))).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        let _ = fs::remove_dir_all(dist);
    }

    /// Client 10.0.0.1 sends four requests that fail validation, then two runs within its limit
    /// of 2, and its third run gets 429. Client 10.0.0.2 can still run.
    #[tokio::test]
    async fn run_rate_limit_counts_valid_runs_per_client() {
        let dist = temp_dir("rate");
        let app = router(state(&dist, 2));
        let from = |client: u8, body: String| {
            let mut request = run(body);
            let address = SocketAddr::from(([10, 0, 0, client], 40000));
            request.extensions_mut().insert(ConnectInfo(address));
            request
        };
        let valid = r#"{"rust": "fn main() {}"}"#.to_string();
        let invalid = [
            "not json".to_string(),
            "{}".to_string(),
            json!({ "rust": "x".repeat(MAX_RUN_SOURCE_BYTES + 1) }).to_string(),
            "x".repeat(MAX_BODY_BYTES + 1),
        ];
        for body in invalid.into_iter().chain([valid.clone(), valid.clone()]) {
            let (status, _, _) = send(&app, from(1, body)).await;
            assert_ne!(status, StatusCode::TOO_MANY_REQUESTS);
        }
        let (status, _, body) = send(&app, from(1, valid.clone())).await;
        assert_eq!(
            (
                status,
                serde_json::from_slice::<Value>(&body).unwrap()["error"].clone()
            ),
            (
                StatusCode::TOO_MANY_REQUESTS,
                json!("run rate limit exceeded")
            )
        );
        let (status, _, _) = send(&app, from(2, valid)).await;
        assert_ne!(status, StatusCode::TOO_MANY_REQUESTS);
        let _ = fs::remove_dir_all(dist);
    }

    /// Fills the table with clients that ran in this minute. A new client gets no run until the
    /// minute of the others is over, and the table then forgets them.
    #[test]
    fn run_rate_table_stays_bounded() {
        let rates = RunRates::default();
        for client in 0..MAX_RATE_LIMITED_CLIENTS as u32 {
            assert!(try_run_rate_limit(&rates, client.to_be_bytes().into(), 1));
        }
        let new = IpAddr::from([255; 4]);
        assert!(!try_run_rate_limit(&rates, new, 1));
        for (started, _) in rates.lock().unwrap().values_mut() {
            *started = started.checked_sub(RATE_LIMIT_WINDOW).unwrap();
        }
        assert!(try_run_rate_limit(&rates, new, 1));
        assert_eq!(rates.lock().unwrap().len(), 1);
    }

    #[test]
    fn run_rate_limit_counts_an_ipv6_network_as_one_client() {
        let rates = RunRates::default();
        let address = |text: &str| text.parse::<IpAddr>().unwrap();
        assert!(try_run_rate_limit(&rates, address("2001:db8::1"), 1));
        assert!(!try_run_rate_limit(&rates, address("2001:db8::ffff:2"), 1));
        assert!(try_run_rate_limit(&rates, address("2001:db8:0:1::1"), 1));
        assert!(try_run_rate_limit(&rates, address("::ffff:192.0.2.1"), 1));
        assert!(!try_run_rate_limit(&rates, address("192.0.2.1"), 1));
    }

    /// The first request times out after 100 ms while its program sleeps until the 1 s run
    /// timeout. The second request must not start a run before the first program is gone.
    #[tokio::test]
    async fn runs_keep_their_permit_after_the_request_is_gone() {
        let dist = temp_dir("permit");
        let mut state = state(&dist, usize::MAX);
        state.run_deps = dist.clone();
        state.config.request_timeout = Duration::from_millis(100);
        let active_runs = state.active_runs;
        let app = router(state);
        // rustc reads an `--extern` crate only when the code uses it, so an empty file is enough.
        fs::write(dist.join("libtrpl-0.rlib"), "").unwrap();
        let sleep = json!({ "rust": "fn main() { std::thread::sleep(std::time::Duration::from_secs(60)) }" })
            .to_string();

        let (status, _, _) = send(&app, run(sleep.clone())).await;
        assert_eq!(status, StatusCode::REQUEST_TIMEOUT);
        let (status, _, body) = send(&app, run(sleep)).await;
        assert_eq!(
            (
                status,
                serde_json::from_slice::<Value>(&body).unwrap()["error"].clone()
            ),
            (
                StatusCode::SERVICE_UNAVAILABLE,
                json!("too many active runs")
            )
        );
        let deadline = Instant::now() + Duration::from_secs(30);
        while active_runs.load(Ordering::Acquire) > 0 {
            assert!(Instant::now() < deadline, "the run kept its permit");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let _ = fs::remove_dir_all(dist);
    }
}
