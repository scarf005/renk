use std::collections::HashMap;
use std::sync::LazyLock;
use std::{fmt, fs};

static WORDS: LazyLock<Vec<String>> = LazyLock::new(|| Vec::from([String::from("a::b;")]));

#[derive(Debug, Default)]
pub struct Counter {
    seen: HashMap<String, usize>,
}

pub enum Kind {
    Unit,
    Pair(u8, u8),
}

impl Counter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total(&self) -> usize {
        self.seen.values().sum()
    }

    pub fn add(&mut self, word: &str) {
        *self.seen.entry(word.to_string()).or_default() += 1;
    }

    pub fn log(&self) {
        println!("{:?}", Self::new()); // keep Self::new(); in comments
    }
}

impl fmt::Display for Counter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.total())
    }
}

fn kind_name(kind: &Kind) -> &'static str {
    match kind {
        Kind::Unit => "unit",
        Kind::Pair(..) => "pair",
    }
}

fn read(path: &str) -> std::io::Result<()> {
    let text = fs::read_to_string(path)?;
    let suffix = if text.is_empty() { ".txt" } else { ".xml" };
    let size = if text.len() > 10 {
        2
    } else if text.is_empty() {
        0
    } else {
        1
    };
    println!("{suffix} {size}");
    Ok(())
}

fn units(values: &[i32]) -> Vec<()> {
    values
        .iter()
        .map(|value| {
            drop(*value);
        })
        .collect::<Vec<()>>()
}

fn checked(value: i32) -> Result<(), String> {
    if value < 0 {
        return Err(format!("{value} < 0"));
    }
    Ok(())
}

fn pick(참: bool) -> i32 {
    if 참 { 1 } else { 2 }
}

fn long_body(values: &[i32]) -> std::io::Result<Vec<String>> {
    Ok(values.iter().map(|value| value.to_string()).collect::<Vec<_>>())
}

fn last_call(counter: &mut Counter) {
    counter.add("x");
}

fn main() {
    let mut counter = Counter::new();
    for word in WORDS.iter() {
        counter.add(word);
    }
    last_call(&mut counter);
    counter.log();
    println!("{counter} {}", kind_name(&Kind::Pair(1, 2)));
    println!("{:?} {:?} {}", units(&[1]), checked(1), pick(true));
    let _ = read("/nonexistent");
    let _ = long_body(&[1]);
}
