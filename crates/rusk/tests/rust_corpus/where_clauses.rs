use std::fmt::Debug;

pub trait Show {
    fn show(&self) -> String;

    fn show_with<F>(&self, f: F) -> String
    where
        F: Fn(&str) -> String;
}

pub struct Wrapper<T>
where
    T: Debug,
{
    pub value: T,
}

pub struct Pair<T>(pub T, pub T)
where
    T: Copy;

impl<T> Show for Wrapper<T>
where
    T: Debug + Clone,
{
    fn show(&self) -> String {
        format!("{:?}", self.value)
    }

    fn show_with<F>(&self, f: F) -> String
    where
        F: Fn(&str) -> String,
    {
        f(&self.show())
    }
}

pub fn longest<'a, 'b, T>(
    first: &'a [T],
    second: &'b [T],
) -> usize
where
    'a: 'b,
    T: PartialEq,
{
    first.len().max(second.len())
}

pub fn apply<F>(f: F) -> u8
where
    F: Fn(
        u8,
        u8,
    ) -> u8,
{
    f(1, 2)
}

pub fn trailing_where<T>(value: T) -> T where
    T: Clone,
{
    value.clone()
}

pub mod nested {
    pub fn inner<T>(value: T) -> String
    where
        T: ToString, // bound comment
    { // brace comment
        let where_ = value.to_string();
        where_
    }
}
