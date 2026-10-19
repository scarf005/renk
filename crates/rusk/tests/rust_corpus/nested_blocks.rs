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
