const LIMIT: u8 = 3;

fn wrap(value: u8) -> Result<u8, ()> {
    Ok(value)
}

pub fn arms(value: Option<Option<u8>>, raw: *const u8) -> Result<u8, ()> {
    match value {
        Some(None) => match raw.is_null() {
            true => Err(()),
            false => wrap(LIMIT),
        },
        Some(Some(LIMIT)) => unsafe { // read
            Ok(*raw)
        }
        Some(Some(inner)) => wrap({
            let doubled = inner * 2;
            doubled + 1
        }),
        None => Ok(unsafe {
            *raw
        }),
    }
}

pub fn looping(mut count: u8) -> u8 {
    match count {
        0 => 0,
        _ => loop {
            count -= 1;
            if count == LIMIT {
                break count;
            }
        },
    }
}
