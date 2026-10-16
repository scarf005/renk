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
