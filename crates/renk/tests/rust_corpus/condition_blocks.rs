pub const FLAG: bool = true;

pub enum Kind {
    A,
    B,
}

pub fn drain(items: &mut Vec<i32>, x: Option<i32>, y: bool) -> Option<i32> {
    let mut total = 0;
    while let Some(m) = {
        items.pop()?;
        items.pop()
    } {
        total += m;
    }
    if match x {
        Some(v) => v > 0,
        None => false,
    } || y
    {
        total += 1;
    }
    let picked = match unsafe {
        // while let Some(m) = {
        std::ptr::read(&total)
    } {
        0 => 1,
        _ => 2,
    };
    if let Some(v) = match x {
        Some(v) => Some(v + 1),
        None => None,
    } {
        total += v;
    }
    for n in {
        let start = 0;
        start..2
    } {
        total += n;
    }
    let text = "if match x {";
    if FLAG {
        total += text.len() as i32;
    }
    match Kind::A {
        Kind::A => total += 1,
        Kind::B => {}
    }
    while let Some(v) = items.pop() {
        total += v;
    }
    Some(total + picked)
}
