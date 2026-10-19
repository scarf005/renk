macro_rules! pick {
    ($operation:expr, {
        $($t:tt)*
    }) => {
        $operation
    };
    ($tokens:ident
        ($($b1:tt)*) // ($($a1:tt)*
        ($($a1:tt)*)
    ) => {
        stringify!($tokens)
    };
    ($single:expr) => { $single };
}

macro_rules! unsize {( $boxed:expr $(,)? ) => ({
    let value = $boxed;
    // a cast (
    unsafe {
        core::ptr::read(&value)
    }
})}

pub fn run() -> (u8, &'static str, u8, u8) {
    (pick!(1, { x y }), pick!(name (a) (b)), pick!(3), unsize!(4))
}

macro_rules! keep {
    ($($tokens:tt)*) => {
        $($tokens)*
    };
}

keep! {
    pub fn visit<A>(value: A) -> A
    where
        A: Copy,
    {
        value
    }
}
