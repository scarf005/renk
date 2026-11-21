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
fn long_view_chains_of_struct_literals_transpile_in_linear_time() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let convert = |links: usize| {
        let links = (0..links)
            .map(|_| "            .map(|x| Foo { a: x })\n")
            .collect::<String>();
        let renk = format!("fn f():\n    view! {{\n        <s value=items\n{links}        />\n    }}\n");
        let start = std::time::Instant::now();
        transpile(&renk).unwrap();
        start.elapsed()
    };
    let (short, long) = (fastest(|| convert(250)), fastest(|| convert(1_000)));
    // Four times the links take about four times as long, not sixty-four.
    assert!(long < short * 8, "{short:?} {long:?}");
}

#[test]
fn long_view_lists_of_multi_line_literals_transpile_in_linear_time() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let convert = |items: usize| {
        let items = "            Foo {\n            },\n".repeat(items);
        let renk = format!("fn f():\n    view! {{\n        <s value=[\n{items}        ] />\n    }}\n");
        let start = std::time::Instant::now();
        transpile(&renk).unwrap();
        start.elapsed()
    };
    let (short, long) = (fastest(|| convert(250)), fastest(|| convert(1_000)));
    // Four times the items take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}

#[test]
fn long_view_lines_of_keyword_tags_transpile_in_linear_time() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let convert = |tags: usize| {
        let tags = "match>".repeat(tags);
        let renk = format!("fn f():\n    view! {{\n        <p>{tags}</p>\n    }}\n");
        let start = std::time::Instant::now();
        transpile(&renk).unwrap();
        start.elapsed()
    };
    let (short, long) = (fastest(|| convert(5_000)), fastest(|| convert(20_000)));
    // Four times the tags take about four times as long, not sixteen.
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
        // `fs::read` keeps its `::`, as `fs.read` would read the `fs` value, and the last `n.set`
        // keeps its `;`.
        let rust = format!(
            "fn f(n: u8) {{\n    view! {{\n        <b on:click=move |_| {{\n            let fs = 0;\n            fs::read(fs);\n{statements}        }}></b>\n    }}\n}}\n"
        );
        let start = std::time::Instant::now();
        rust_to_renk(&rust).unwrap();
        start.elapsed()
    };
    let (short, long) = (fastest(|| convert(50)), fastest(|| convert(200)));
    // Four times the statements take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}

#[test]
fn view_blocks_with_a_kept_last_semicolon_convert_about_as_fast_as_others() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let convert = |last: &str| {
        let statements = (0..200)
            .map(|n| format!("            n.set({n});\n"))
            .collect::<String>();
        let rust = format!(
            "fn f(n: u8) {{\n    view! {{\n        <b on:click=move |_| {{\n{statements}            {last}\n        }}></b>\n    }}\n}}\n"
        );
        let start = std::time::Instant::now();
        rust_to_renk(&rust).unwrap();
        start.elapsed()
    };
    let (value, statement) = (fastest(|| convert("n.get()")), fastest(|| convert("n.set(0);")));
    // The last `n.set(0);` keeps its `;`, as without it the block returns a value. Finding that
    // line takes one more lowering of the block, not one per halving of its 201 lines.
    assert!(statement < value * 2, "{value:?} {statement:?}");
}

#[test]
fn many_view_blocks_with_colon_paths_convert_in_linear_time() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let convert = |views: usize| {
        // In each `view!`, `fs::read` keeps its `::`, as `fs` is a parameter, and the
        // `std::mem::drop` paths around it become `std.mem.drop`.
        let views = (0..views)
            .map(|n| {
                format!(
                    "    view! {{\n        <b on:click=move |_| {{\n            std::mem::drop({n});\n            fs::read(fs);\n            std::mem::drop({n});\n        }}></b>\n    }};\n"
                )
            })
            .collect::<String>();
        let rust = format!("fn f(fs: u8) {{\n{views}}}\n");
        let start = std::time::Instant::now();
        let renk = rust_to_renk(&rust).unwrap();
        let elapsed = start.elapsed();
        assert!(renk.contains("            std.mem.drop(0)\n            fs::read(fs)\n"), "{renk}");
        elapsed
    };
    let (short, long) = (fastest(|| convert(25)), fastest(|| convert(100)));
    // Four times the statements take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}

/// The time to convert a `view!` body of `count` copies of `tag` from Rust to Renk and back, once
/// with all tags on one line and once with one tag per line.
fn convert_view_tags(tag: &str, count: usize) -> std::time::Duration {
    [" ", "\n        "]
        .iter()
        .map(|separator| {
            let body = vec![tag; count].join(separator);
            let rust = format!("fn f() {{\n    view! {{\n        {body}\n    }}\n}}\n");
            let start = std::time::Instant::now();
            transpile(&rust_to_renk(&rust).unwrap()).unwrap();
            start.elapsed()
        })
        .sum()
}

#[test]
fn long_view_bodies_of_closure_attributes_convert_in_linear_time() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // Each `move |_| a` body ends at the next `,`, `;`, or closing bracket, and markup has none.
    let tag = "<b on:click=move |_| a></b>";
    let (short, long) = (
        fastest(|| convert_view_tags(tag, 50)),
        fastest(|| convert_view_tags(tag, 200)),
    );
    // Four times the tags take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}

#[test]
fn long_view_bodies_of_nested_elements_convert_in_linear_time() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let tag = "<p><i><b>x</b></i></p>";
    let (short, long) = (
        fastest(|| convert_view_tags(tag, 100)),
        fastest(|| convert_view_tags(tag, 400)),
    );
    // Four times the tags take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}

#[test]
fn long_view_bodies_of_spaced_tag_ends_convert_in_linear_time() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // Each tag ends at the spaced `>` after its closure value, and the text after it holds a
    // bare `>`.
    let tag = "<b on:click=move |_| go() > Count > more </b>";
    let (short, long) = (
        fastest(|| convert_view_tags(tag, 50)),
        fastest(|| convert_view_tags(tag, 200)),
    );
    // Four times the tags take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}

#[test]
fn long_view_bodies_of_closures_starting_with_literals_convert_in_linear_time() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // Each body of `move ||"a"` starts with a string. The body scan skips the string and tests
    // no index inside it, so a scan from an earlier body must find a memo past the string.
    let tag = "<b class=move ||\"a\"></b>";
    let (short, long) = (
        fastest(|| convert_view_tags(tag, 50)),
        fastest(|| convert_view_tags(tag, 200)),
    );
    // Four times the tags take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}
