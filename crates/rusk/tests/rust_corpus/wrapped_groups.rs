use std::fmt::{self, Debug, Write};

macro_rules! pick {
    ($first:expr => $second:expr) => {
        $second
    };
}

pub struct Runner {
    fwd: Option<
        Box<dyn Fn(u8) -> u8>,
    >,
}

pub unsafe trait Marker {
    fn id(&self) -> u8;
}

unsafe impl Marker for Runner {
    fn id(&self) -> u8 {
        1
    }
}

pub fn case<
    T: Debug + Clone,
>(
    value: T,
) -> Result<(
    T,
    T,
), ()> {
    Ok((value.clone(), value))
}

pub fn shapes(runner: &Runner, left: u8, right: u8) -> u8 {
    let total = u8::checked_add(left,
        right).unwrap_or(0);
    let picked = pick! {total =>
        total + 1
    };
    let less = left <
        right;
    let applied = match runner.fwd.as_ref() {
        Some(fwd) => Some(0).map(|v| {
            fwd(v) + picked
        }),
        None => match less {
            true => {
                let doubled = picked * 2;
                Some(doubled)
            }
            false => None,
        },
    };
    applied.unwrap_or(runner.id())
}

pub fn flags(out: &mut String, items: &[bool]) -> fmt::Result {
    for item in items {
        match item {
            true => out.write_str("t"),
            false => out.write_str("f"),
        }?;
    }
    Ok(())
}
