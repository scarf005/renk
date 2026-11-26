macro_rules! make_message {
    ($name:expr) => { format!("hello {}", $name) };
}

pub fn make_message(name: &str) -> String {
    make_message!(name)
}

pub fn normalized_names(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_ascii_lowercase()) }
        })
        .collect::<Vec<String>>()
}

pub fn called_closures(A: u8) -> u8 {
    let a = (|| {
        A + "})()".len() as u8 // })()
    })();
    let b = [(|X: u8| {
        X
    })(a)];
    std::convert::identity((|| {
        b[0]
    })())
}

pub fn parameter_lines(limit: u8) -> u8 {
    let start = |first: u8,
                 second: u8| {
        // The bars stay on the parameter lines.
        first + second
    };
    let add = |first: u8,
               second: u8| first + second;
    let pair = move |(first, second): (u8, u8),
                     third: u8| {
        first * second + third
    };
    let triple = |first: &mut u8, // |
                  second: u8,
                  third: u8| {
        *first + second + third
    };
    let text = "|a,";
    let step: fn(u8) -> u8 = match limit {
        0 | 1 => |a| a,
        _ => |a| a + 1,
    };
    start(add(1, 2), pair((3, 4), step(limit))) + text.len() as u8 + triple(&mut 1, 2, 3)
}
