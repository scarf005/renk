mod private {
    pub trait Sealed {}
}

pub struct Point { pub x: u8 }
impl Point { pub fn origin() -> Self { Point { x: 0 } } }
impl<T> private::Sealed for Vec<T> {}

pub enum Link<'a, T> {
    Borrowed {
        value: &'a T,
        rest: Vec<T>,
    },
    Owned(T),
}

macro_rules! nothing { ($($tokens:tt)*) => {}; }
nothing! { fn hidden() {} }

macro_rules! make {
    (
        impl $name:ident {
            $field:ident
        }
    ) => {
        pub struct $name { pub $field: u8 }
    };
}
make! { impl Made { x } }

#[derive(PartialEq)]
pub struct Key { pub k: u8 }

pub fn get(key: Key) -> Option<u8> { Some(key.k) }

pub fn lookup(k: u8, made: Made) -> u8 {
    let found = match get(Key { k }) {
        Some(v) => v,
        None => made.x,
    };
    let picked = if get(Key { k }).is_some() {
        found
    } else {
        0
    };
    picked
}

mod empty {}
pub mod one_line { pub fn f() -> u8 { 1 } }
