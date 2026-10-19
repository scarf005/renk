use std::cell::RefCell;

thread_local! {
    static DROP_VECTOR: RefCell<Vec<i32>> = RefCell::new(vec![0; 4]);
}

pub struct Droppable {
    pub k: usize,
}

pub struct Mismatch {
    pub got: Option<u8>,
    pub wanted: u8,
}

macro_rules! debug_fmt_fields {
    ($tyname:ident, $($($field:tt/*TODO a tuple element*/).+),*) => {
        fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
            f.debug_struct(stringify!($tyname))
                $(
              .field(stringify!($($field).+), &self.$($field).+)
              )*
              .finish()
        }
    }
}

impl std::fmt::Debug for Droppable {
    debug_fmt_fields!(Droppable, k);
}

impl Droppable {
    pub fn new(k: usize) -> Droppable {
        DROP_VECTOR.with(|slot| {
            slot.borrow_mut()[k] += 1;
        });
        DROP_VECTOR.with(|slot| { slot.borrow_mut()[k] -= 1; });
        let made = Droppable { k };
        made
    }
}

pub fn operands(a: bool, b: usize) -> usize {
    let length = usize::from(a)
        + if a {
            b + 1
        } else {
            b
        }
        - 1;
    let mut has = false;
    if (a || {
        has = b > 1;
        has
    }) && b > 2
        && !has
    {
        return 0;
    }
    while (a && {
        has = !has;
        has
    }) {}
    let table = [b, {
        let doubled = b * 2;
        doubled
    }];
    let call = usize::max(b, {
        let tripled = b * 3;
        tripled
    });
    let commented = b // a || {
        + 1;
    let quoted = String::from("if (a || {") + "[b, {";
    length + table[1] + call + commented + quoted.len()
}

pub fn strip(items: &mut std::slice::Iter<'_, u8>, prefix: &[u8]) -> Result<usize, Mismatch> {
    match prefix.iter().try_for_each(|wanted| match items.next() {
        Some(got) if got == wanted => Ok(()),
        got => Err(Mismatch { got: got.copied(), wanted: *wanted }),
    }) {
        Ok(()) => Ok(prefix.len()),
        Err(mismatch) => Err(mismatch),
    }
}

pub fn unions(items: Vec<u8>) -> usize {
    let mut union =
        items.clone();
    union.push(1);
    union = union.into_iter().rev().collect();
    union.len()
}

pub fn operand_conditions(a: bool, mut has: bool) -> bool {
    if a || {
        has = !has;
        has
    } {
        return true;
    }
    while !a && {
        has = !has;
        has
    } {}
    if [a].iter().any(|&b| {
        b || has
    }) {
        return false;
    }
    has
}

pub fn closure_operands(a: bool, kind: u8, items: &[u8]) -> bool {
    if match kind {
        0 => {
            a
                && items.iter().any(|item| match item {
                    0 => false,
                    _ => true,
                })
        }
        _ => true,
    } {
        return true;
    }
    a
        || items.iter().all(|item| {
            *item > 1
        })
}

pub fn wrapped_conditions(a: u8, c: char) -> u8 {
    if (matches!(
        a,
        1 | 2
    ) && matches!(c, '<' | '='))
    {
        return 2;
    } else {
        std::mem::drop(c);
    }
    while [
        a,
        1
    ] == [0, 1]
    {
        return 3;
    }
    let wide = if (matches!(
        a,
        3
    ) || a > 4)
    {
        2
    } else {
        0
    };
    let pair = (
        a,
        c,
    );
    {
        let _ = pair;
    }
    std::mem::drop(
        a
    );
    {
        return wide;
    }
}

pub fn value_scrutinees(a: Option<u8>, b: Option<u8>) -> u8 {
    if let Some(found) = if let Some(x) = a {
        Some(x)
    } else if let Some(y) = b {
        Some(y)
    } else {
        None
    } {
        return found;
    }
    while let Some(next) = match a {
        Some(0) => None,
        other => other,
    } {
        return next;
    }
    let Some(last) = (if let Some(x) = b { Some(x) } else { a }) else {
        return 0;
    };
    last
}
