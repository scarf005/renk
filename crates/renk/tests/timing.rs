use renk::{rust_to_renk, transpile};

// The tests here run one at a time, apart from the heavier property tests, so they measure
// little load besides their own.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The fastest of three runs, so other load on the machine skews a time ratio less.
fn fastest(run: impl Fn() -> std::time::Duration) -> std::time::Duration {
    (0..3).map(|_| run()).min().unwrap()
}

#[test]
fn long_groups_convert_in_linear_time() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let convert = |rows: usize| {
        let rows = (0..rows)
            .map(|n| format!("    ({n}, {n}),\n"))
            .collect::<String>();
        let rust = format!("pub static TABLE: &[(u32, u32)] = &[\n{rows}];\n");
        let start = std::time::Instant::now();
        transpile(&rust_to_renk(&rust).unwrap()).unwrap();
        start.elapsed()
    };
    let (short, long) = (fastest(|| convert(5_000)), fastest(|| convert(20_000)));
    // Four times the rows take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}

#[test]
fn long_block_argument_chains_convert_in_linear_time() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let convert = |links: usize| {
        let links = (0..links)
            .map(|n| format!("        .a(|y| {{\n            y + {n}\n        }})\n"))
            .collect::<String>();
        let rust = format!("fn main() {{\n    x\n{links}        .z();\n}}\n");
        let start = std::time::Instant::now();
        rust_to_renk(&rust).unwrap();
        start.elapsed()
    };
    let (short, long) = (fastest(|| convert(1_000)), fastest(|| convert(4_000)));
    // Four times the links take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}

#[test]
fn deep_closures_convert_in_time_linear_in_their_text() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let convert = |depth: usize| {
        let closures = (0..depth).fold("1".to_string(), |body, level| {
            let indent = "    ".repeat(depth - level);
            format!("g(|| {{\n{indent}    {body}\n{indent}}})")
        });
        let rust = format!("fn f() -> u8 {{\n    {closures}\n}}\n");
        let start = std::time::Instant::now();
        rust_to_renk(&rust).unwrap();
        start.elapsed()
    };
    // Twice the depth is four times the indented text, so it should take about four times as
    // long, not eight; six sits between them.
    let (short, long) = (fastest(|| convert(200)), fastest(|| convert(400)));
    assert!(long < short * 6, "{short:?} {long:?}");
}

#[test]
fn long_view_blocks_convert_in_linear_time() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let convert = |statements: usize| {
        let statements = (0..statements)
            .map(|n| format!("            n.set({n});\n"))
            .collect::<String>();
        let rust = format!(
            "fn f(n: u8) {{\n    view! {{\n        <b on:click=move |_| {{\n{statements}        }}></b>\n    }}\n}}\n"
        );
        let start = std::time::Instant::now();
        rust_to_renk(&rust).unwrap();
        start.elapsed()
    };
    let (short, long) = (fastest(|| convert(50)), fastest(|| convert(200)));
    // Four times the statements take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}
