pub fn classify(values: &[u32], limit: u32) -> &'static str {
    let mut index = 0;
    while index < values.len()
        && values[index] < limit
    {
        index += 1;
    }
    if values.is_empty()
        || values.iter().all(|value| *value == 0)
    {
        "empty"
    } else if values.len() > 3
        && values[0] > limit // first
    { // many
        "many"
    } else {
        "few"
    }
}

pub struct Scanner {
    pub text: String,
}

impl Scanner {
    pub fn braces(&self) -> usize {
        let pattern = "
{
";
        match self
            .text
            .matches(pattern)
            .count()
        {
            0 => 0,
            count => count,
        }
    }
}

pub fn else_if_own_line(a: bool, b: bool, c: u8) -> u8 {
    if a
        || b
    {
        1
    } else if b {
        2
    } else if c == 3 || c == 4 || c == 5 || c == 6 || c == 7 || c == 8 || c == 9 || c == 10 || c == 1
    {
        3
    } else {
        4
    }
}

const UPPER: bool = true;

pub fn else_if_neighbours(s: &str) -> u8 {
    if UPPER
        && s == "{"
    {
        1
    } else if s.ends_with("}") // {
    {
        2
    } else if matches!(s, "(" | "[")
        || (UPPER && s.len() > 3)
    {
        3
    } else if s.is_empty() { 4 } else {
        5
    }
}

pub fn Own_line_neighbours(v: Option<u8>) -> u8 // {
{
    v.map(|n| {
        n + 1
    })
    .unwrap_or(0)
}
