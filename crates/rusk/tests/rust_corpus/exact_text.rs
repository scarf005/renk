pub fn blank_lines(c: bool) -> u8 {
    let a = 1;

    let b = 2;
    if c {
        a
    } else if !c {
        b
    } else {
        // the last branch
        2
    }
}

pub fn after_blank() -> u8 {
    let x = "line one

line three";
    x.len() as u8
}

pub fn block_arms(o: Option<u8>, v: &mut Vec<u8>) -> u8 {
    match o {
        Some(_) => {}
        None => {}
    }
    match o {
        Some(x) => {
            v.push(x);
        }
        None => {
            v.clear();
        }
    }
    match o {
        Some(x) => v.push(x),
        None => {}
    };
    let n = match o {
        Some(x) => {
            let y = x + 1;
            y
        }
        None => 0,
    };
    n
}
