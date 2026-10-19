macro_rules! twice {
    ($t:expr) => {
        $t + $t
    };
}

macro_rules! sum_all {
    () => {
        0
    };
    ($head:expr $(, $rest:expr)*) => {{
        let mut total = $head;
        $(
            total += $rest;
        )*
        total
    }};
}

pub fn use_macros() -> i32 {
    twice!(2) + sum_all!(1, 2, 3)
}

macro_rules! one {() => {
    1
}} /* end } */

macro_rules! two {() => {
    2
}} // end }

pub fn use_inline_arms() -> i32 {
    one!() + two!()
}
