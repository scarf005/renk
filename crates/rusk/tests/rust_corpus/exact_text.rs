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
