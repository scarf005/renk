fn wrap(n: i32) -> Option<i32> {
    Some(n)
}

pub fn call_wrapped_match(x: Option<i32>) -> Option<i32> {
    let v = wrap(match x {
        Some(v) => v,
        None => 0,
    })?;
    Some(v)
}

pub fn call_wrapped_else(c: bool) -> Option<i32> {
    let v = wrap(if c {
        1
    } else {
        2
    })?;
    Some(v)
}

pub struct Table {
    pub fs: [fn(i32) -> i32; 1],
}

pub fn macro_receiver_index_call(t: Table) -> i32 {
    let a = vec![t].remove(0).fs[0](1);
    a
}
