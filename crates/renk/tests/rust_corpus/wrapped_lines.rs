use std::{
    cmp::Ordering,
    collections::HashMap,
    sync::LazyLock,
};

static TABLE: LazyLock<HashMap<&'static str, usize>> =
    LazyLock::new(|| HashMap::from([("alpha", 1), ("beta", 2), ("gamma", 3)]));

const SCRIPT: &str = r#"#!/bin/sh
if [ "$1" = "main" ]; then
    echo "blocked"
fi
"#;

pub fn describe(name: &str, verbose: bool) -> String {
    let value = TABLE.get(name).copied().unwrap_or_default();
    let label = format!(
        "{name}: {value} {}",
        if verbose { "(verbose)" } else { "" }
    );
    label
}

pub fn sorted(mut entries: Vec<(String, Option<usize>)>) -> Vec<(String, Option<usize>)> {
    entries.sort_by(
        |(a, a_value), (b, b_value)| match (a_value.is_some(), b_value.is_some()) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.cmp(b),
        },
    );
    entries
}

pub fn command_args(path: &str) -> Vec<String> {
    let mut args = Vec::new();
    args.extend([
        "--power".to_string(),
        "default-file".to_string(),
        format!("--write={path}"),
    ]);
    args.push(SCRIPT.lines().count().to_string());
    args
}

pub fn first_names(names: &[&str]) -> Option<Vec<String>> {
    Some(
        names
            .iter()
            .map(|name| name.to_uppercase())
            .collect(),
    )
}

pub struct Span {
    pub start: usize,
    pub end: usize,
}

pub fn span(range: Option<(usize, usize)>, names: Option<Vec<String>>) -> (Span, usize) {
    let fallback = Span {
        start: 0,
        end: 0,
    };
    let span = match range {
        Some((start, end)) => Span {
            start,
            end,
        },
        None => fallback,
    };
    let count = match names {
        Some(names) => names
            .iter()
            .filter(|name| !name.is_empty())
            .count(),
        None => 0,
    };
    (span, count)
}

pub type Pair<T> = (T, T);

pub struct Unit {}

pub struct Wrapper(pub Vec<String>);

pub fn summed(
    values: &[i32],
    offset: i32,
) -> Result<i32, String> {
    let total = std::thread::scope(|s| {
        let handle = s.spawn(|| values.iter().sum::<i32>());
        handle.join().map_err(|_| "panic".to_string())
    })?;
    Ok(total + offset)
}

pub fn classify(values: &[Option<i32>], flag: bool) -> (usize, &'static str, Unit) {
    let positive = values
        .iter()
        .filter(|value| match value {
            Some(v) => *v > 0,
            None => false,
        })
        .count();
    let label = if flag {
        "flag"
    } else {
        "plain"
    };
    (positive, label, Unit {})
}

pub fn first_word(text: &str) -> Option<String> {
    let Some(word) = text
        .split_whitespace()
        .next()
    else {
        return None;
    };
    Some(word.to_string())
}

pub fn wrapped(names: &[&str]) -> Wrapper {
    let names: Vec<String> = [
        "first",
        "second",
    ]
    .map(str::to_owned)
    .into();
    Wrapper(names.into_iter().chain(names_from(names_len(&[]))).collect())
}

fn names_from(count: usize) -> Vec<String> {
    vec![String::new(); count]
}

fn names_len(names: &[&str]) -> usize {
    names.len()
}

pub struct Handlers {
    pub calls: Vec<fn(usize) -> usize>,
}

pub fn dispatch(handlers: &Handlers, index: usize) -> usize {
    let total = handlers.calls[index](index) + (handlers.calls[0])(index);
    let parsed = "7".parse::<usize>().unwrap_or_default();
    total + parsed + [1usize, 2].iter().sum::<usize>()
}
