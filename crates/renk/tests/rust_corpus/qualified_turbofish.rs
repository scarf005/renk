use std::future::Future;
use std::ops::Add;

fn assert_future<T, F: Future<Output = T>>(f: F) -> F {
    f
}

pub fn wrap<I, F>(ret: F) -> F
where
    I: Iterator,
    I::Item: Future,
    F: Future<Output = (<I::Item as Future>::Output, usize)>,
{
    // assert_future::<(<I::Item as Future>::Output, usize), _>(ret)
    assert_future::<(<I::Item as Future>::Output, usize), _>(ret)
}

#[allow(non_snake_case)]
pub fn paths(xs: &[u32], i: u8) -> usize {
    let items = Vec::<<Vec<u8> as IntoIterator>::Item>::new();
    let sums = std::iter::empty::<<u8 as Add>::Output>().count();
    let zero = <u8 as Default>::default();
    let text = "empty::<<u8 as Add>::Output>()";
    let BITS = xs[i as usize].count_ones() as usize;
    items.len() + sums + zero as usize + text.len() + BITS
}
