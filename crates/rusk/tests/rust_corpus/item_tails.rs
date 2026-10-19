#[derive(Clone, Copy)]
pub union Bits {
    pub byte: u8,
    pub flag: bool,
}

macro_rules! twice {
    ($value:expr => $other:expr) => {
        $value * $other
    };
}

pub fn scan(mut count: u8, ready: bool) -> Result<u8, ()> {
    while count > 0 {}
    while count > 3
        && count % 7 != 0
    {}
    let picked = if ready {
        Ok(count)
    } else {
        Err(())
    }?;
    let doubled = Some(twice! {picked =>
        2
    });
    Ok(doubled.unwrap_or(0))
}
