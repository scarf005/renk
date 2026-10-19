pub fn ranges(s: &[u8], start: u32, end: u32, v: &[u32]) -> usize {
    let a = &s[start as usize..end as usize];
    let b = &s[start as usize..v[0] as usize];
    let mut n = 0;
    for i in start as usize..v.len() {
        n += i;
    }
    let c = (start as u64).pow(2) as usize;
    let d = v.len() as u32 as usize;
    let e = start as usize..=end as usize;
    let f = start as usize..V::MAX.len(); // as usize..
    let t = " as usize..v.len()";
    a.len() + b.len() + n + c + d + e.count() + f.count() + t.len()
}

pub struct V;

impl V {
    pub const MAX: [u8; 3] = [0; 3];
}
