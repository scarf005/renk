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

pub fn multiline_literals() -> usize {
    let text = "tail  
semicolon; // not a comment
  indented";
    let raw = r#"quote " and {  
"#;
    text.len() + raw.len()
}

pub fn let_else(v: Option<i32>) -> i32 {
    let Some(x) = v else {
        return 0;
    };
    let Some(y) = v.map(|n| n + 1) else { return 1 };
    if x > y {
        let Some(z) = v else {
            eprintln!("none"); // let a = b else {
            return 2;
        };
        z
    } else {
        x
    }
}

pub fn trailing_if(n: i32) -> i32 {
    let size = if n > 10 {
        1
    } else if n < 0 {
        // let a = if b {
        2
    } else {
        3
    };
    let t = if n > 0 { 1 } else { 2 };
    let mut total = if i32::MAX > n {
        let inner = if n % 2 == 0 {
            n
        } else {
            -n
        };
        inner
    } else {
        0
    };
    total += if size == t { 1 } else { 0 };
    return if total == 0 {
        size + t
    } else {
        total
    };
}
