pub struct Names(pub Vec<String>);

pub fn count(v: &[u32], names: &Names, c: bool) -> usize {
    let mut n = 0;
    for x in v.iter() { n += *x as usize; }
    for name in names.0.iter() { n += name.len(); }
    while n > 100 { n -= std::cmp::max(1, 2); }
    if v.is_empty() { return 0; }
    if c { n += Vec::<u32>::new().len(); } // {
    loop { if n > 0 { break; } n += 1; }
    if names.0.is_empty() { n } else { n + names.0[0].len() }
}
