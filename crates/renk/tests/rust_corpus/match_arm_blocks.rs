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

pub fn guarded(value: Option<(u8, u8)>, limit: u8) -> u8 {
    match value {
        Some((first, second))
            if first == limit =>
        {
            let total = first + second;
            total
        }
        Some((
            first,
            _,
        ))
            if first > limit => first,
        Some((_, second)) if second > limit => {
            let doubled = second * 2;
            doubled
        }
        Some(pair)
            // The pair is below the limit.
            if pair.0 < limit => pair.1,
        _ => 0,
    }
}

pub struct Span {
    pub start: u8,
    pub end: u8,
}

pub fn spans(span: Span) -> u8 {
    match span {
        Span { start, .. }
            if start > 1 =>
        {
            start
        }
        Span { end, .. }
            if end > 1 => end,
        _ => 0,
    }
}
