pub fn letters(c: char) -> u8 {
    let class = match c {
        'a' | 'e'
        | 'i' => 1,
        'b'
        | 'c'
        | 'd' => 2,
        _ => 0,
    };
    let add = |x: u8| x + class;
    let zero = || 0;
    if let 'x'
    | 'y' = c
    {
        return zero();
    }
    add(1)
}

pub fn pick(ready: bool, count: u8) -> u8 {
    let value = if ready {
        count
    }
    // Not ready yet.
    else {
        0
    };
    if count > 3 {
        value
    } /* past the limit */
    else if count > 1 {
        value + 1
    }
    // Neither.
    /* Fall back. */
    else {
        value + 2
    }
}

pub fn chained(v: Option<u8>, items: Vec<u8>) -> Result<Option<u8>, ()> {
    let first = v.map(|x| {
        x + 1
    })
    .or(Some(0));
    let total: u8 = items.iter().map(|x| {
        x * 2
    })
    .filter(|x| *x > 1)
    .sum();
    let checked = Some(total).ok_or(()).map(|t| {
        t + 1
    })
    ?;
    let listed = [
        checked,
        total,
    ]
    .len();
    v.map(|x| {
        x + listed as u8
    })
    .or(first)
    .ok_or(())
    .map(Some)
}

pub fn sizes(items: &[u8], extra: usize, place: &mut usize) -> usize {
    let wide = items.len()
    - extra;
    let same = wide
    == extra;
    *place = wide;
    let reset = || 0;
    // A sum over several lines.

    items.len() * 2
    + extra
    // The next term counts the flag.
    + usize::from(same)

    + reset()
}

pub fn sizes_tail(items: &[u8]) -> usize {
    let call = || items.len();
    call();
    || 1;
    call()
}
