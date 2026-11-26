pub fn leading_bars(x: i32, flag: bool) -> i32 {
    let add = |v: i32| v + 1;
    let base = match x {
        1 => 10,
        | 2 | 3 => 20,
        4 if flag => 40,
        | 5
        | 6 => 50,
        7 => (|y: i32| y + 1)(7),
        | 8 => (|| add(8))(),
        9 => "=>".len() as i32, // =>
        | 10 => 100,
        _ => 0,
    };
    let nested = match x {
        1 => match base {
            0 => 1,
            | 1 | 2 => 2,
            _ => 3,
        },
        | 2 => 4,
        _ => 0,
    };
    let bits = match x {
        1 => base
            | nested,
        | 2 => add(base) | 1,
        _ => 0,
    };
    let picks: [fn(i32) -> i32; 2] = [
        |v| v + 1,
        |v| v * 2,
    ];
    picks[0](base) + nested + bits
}

pub fn split_patterns(word: &str) -> u8 {
    match word {
        "=>" // =>
        | "a" => 1,
        | "b"
        | "c" => 2,
        _ => 0,
    }
}
