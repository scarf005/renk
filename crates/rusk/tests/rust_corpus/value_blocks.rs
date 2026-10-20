use std::future::Future;

async fn one() -> i32 {
    1
}

fn spawn<F: Future<Output = i32>>(future: F) -> F {
    future
}

pub fn unit_function_with_value_blocks(flag: bool, values: &mut Vec<i32>) {
    let future = spawn(async {
        let first = one().await;
        first + 1
    });
    let moved = spawn(async move {
        one().await
    });
    let doubled = values.iter().map(|value| {
        let twice = value * 2;
        twice + 1
    }).count();
    let chosen = if flag || values.is_empty() {
        1
    } else {
        2
    };
    let matched = match chosen {
        1 => {
            let base = 10;
            base + 1
        }
        _ => 0,
    };
    let block = {
        let inner = 3;
        inner * 2
    };
    let typed = |value: i32| -> i32 {
        value + 1
    };
    if flag || chosen > 1 {
        values.push(chosen);
    }
    values.iter().for_each(|value| {
        println!("{value}");
    });
    let _ = (future, moved, doubled, matched, block, typed(1));
}

pub fn empty_value_blocks(A: u8) -> u8 {
    let unit = {
    };
    let commented = { // {
        /* } */
    };
    let _ = (unit, commented);
    A
}

pub fn let_values_with_blocks_in_headers(x: u8, items: &[u8]) -> u8 {
    let v = if match x {
        0 => true, // {
        A_MAX => false,
        _ => false,
    } {
        1
    } else {
        2
    };
    let r = match items.iter().position(|t| {
        *t == x // }) {
    }) {
        Some(at) => at as u8,
        None => 0,
    };
    v + r
}

const A_MAX: u8 = 9;
