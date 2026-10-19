pub fn pick(c: bool, d: bool) -> u8 {
    let x =
        if c && d {
            1
        } else if d {
            2
        } else {
            3
        };
    x
}

pub fn first(a: Option<u8>) -> u8 {
    if let Some(m) = a {
        m
    } else {
        0
    }
}

pub struct Wrap<K, V>(Vec<(K, V)>);
pub fn raw<K, V>(entries: &[(K, V)]) -> *const [(K, V)] {
    entries as *const [(K, V)] as *const [(K, V)]
}
pub fn view<K, V>(entries: &mut Vec<Wrap<K, V>>) -> *mut Vec<Wrap<K, V>> {
    entries as *mut Vec<Wrap<K, V>>
}
pub fn same(a: &[u8], b: &[u8]) -> bool {
    <[u8] as PartialEq<[u8]>>::eq(a, b) && <Vec<u8> as PartialEq<Vec<u8>>>::eq(&a.to_vec(), &b.to_vec())
}

macro_rules! make(
    ($name:ident) => {
        pub struct $name<T>(Vec<T>);
        impl<T> $name<T> { pub fn len(&self) -> usize { self.0.len() } }
    }
);
make!(Bag);

pub fn total(v: &[u8]) -> usize {
    apply(unsafe {
        let copy: Vec<u8> = v.to_vec();
        copy
    }, |x| { let first: Option<u8> = x.first().copied(); first })
}
fn apply(v: Vec<u8>, f: impl Fn(&[u8]) -> Option<u8>) -> usize {
    f(&v).map_or(0, usize::from)
}

macro_rules! tokens { ($($t:tt)*) => { stringify!($($t)*) }; }
pub fn source() -> &'static str {
    let q = tokens! { fn from() {} };
    q
}

pub enum Event<'a, T> {
    Borrowed { value: &'a Vec<T> },
    #[allow(dead_code)]
    Owned { value: Vec<T>, rest: Option<Box<T>> },
}
pub struct Pair { pub left: Vec<u8>, pub right: Option<Vec<u8>> }

pub fn bits(t: (u8, (u8, u8))) -> u64 {
    0.0f64.to_bits() + 1e3f64.to_bits() + 0x1Fu64.max(u64::from(t.1.0))
}

pub fn run(x: &mut u8, items: &mut Vec<u8>) {
    let set = |v: &mut u8| {
        *v = 1;
    };
    set(x);
    let push = |items: &mut Vec<u8>| {
        (items).push(1);
    };
    push(items);
}

pub fn class(c: char) -> u8 {
    match c {
        'a'
        | 'b'
        // Vowels after consonants.
        | 'e' => 1,
        _ => 0,
    }
}

pub trait Visit {
    fn start(&mut self) {}
    fn name(&self) -> Vec<u8> { Vec::new() }
    fn finish(self) -> u8;
}
impl Visit for u8 { fn finish(self) -> u8 { self.wrapping_add(1) } }

pub fn wrap(c: bool) -> u8 {
    let start = if c
    {
        1
    } else {
        2
    };
    start
}

pub fn arm(x: u8, c: bool) -> Option<u8> {
    Some(match x {
        1 => if c {
            2
        } else if x > 1 {
            4
        } else {
            3
        },
        2 => u8::max(if c {
            2
        } else {
            3
        }, 1),
        _ => 0,
    })
}

pub fn gated() -> u8 {
    #[cfg(any(
        unix,
        windows,
    ))]
    {
        return 1;
    }
    #[allow(unused)]
    {
        let _x = 2;
    }
    0
}

macro_rules! twice {
    // twice!(x);
    ($x:expr) => { $x * 2 };
}
pub fn four() -> u8 {
    twice!(2)
}

pub fn is_small(n: u8) -> bool {
    #[cfg(not(feature = "big"))]
    match n {
        0 => true,
        _ => false,
    }
    #[cfg(feature = "big")]
    n.is_power_of_two()
}
pub fn kind(n: u8) -> u8 {
    #[cfg(unix)]
    if n > 1 {
        1
    } else {
        0
    }
    #[cfg(not(unix))]
    n
}

pub fn both(x: Option<u8>, y: u8) -> bool {
    if match x {
        Some(v) => {
            v > 1
        }
        None => false,
    } && match y {
        0 => true,
        _ => false,
    } {
        return true;
    }
    false
}

pub fn step(x: Option<u8>) -> Result<bool, ()> {
    Ok(match x {
        Some(v) => run_once(|k| match k {
            1 => Ok(v > 0),
            _ => Ok(false),
        })?,
        None => false,
    })
}
fn run_once(f: impl Fn(u8) -> Result<bool, ()>) -> Result<bool, ()> {
    f(1)
}

pub fn parse_all(input: &str) -> usize {
    counter.call_once(input)
}
fn counter(input: &str) -> usize {
    input.len()
}
trait CallOnce {
    fn call_once(self, input: &str) -> usize;
}
impl<F: Fn(&str) -> usize> CallOnce for F {
    fn call_once(self, input: &str) -> usize {
        self(input)
    }
}

pub fn show(items: &[u8]) -> String {
    let mut out = String::new();
    visit(items, &mut |item: &dyn std::fmt::Display| out.push_str(&item.to_string()));
    out
}
fn visit(items: &[u8], f: &mut dyn FnMut(&dyn std::fmt::Display)) {
    for item in items {
        f(item);
    }
}

pub struct Field {
    pub member: String,
}
pub fn field(ident: Option<String>, index: usize) -> Field {
    Field {
        member: match &ident {
            Some(name) => name.clone(),
            None => index.to_string(),
        },
    }
}

pub fn closure_match_operands(xs: Vec<u8>) -> bool {
    let any = xs.iter().any(|x| match x {
        0 => true,
        _ => false,
    }) && xs.len() > 2;
    let count = xs.iter().filter(|x| match x {
        0 => true,
        _ => false, // }) || false
    }).count() + 1;
    any || xs.iter().all(|x| match x {
        1 => true,
        _ => false,
    }) || count > 3
}

pub fn borrowed_match_values(mut e: Vec<u32>) -> u32 {
    let first = &match e
        .iter()
        .next()
    {
        Some(v) => *v,
        None => 0,
    };
    let last = &mut match e
        .iter_mut()
        .map(|value| *value + 1)
        .last()
    {
        Some(v) => v,
        None => 0,
    };
    *last += 1;
    e.push(*first);
    *first + *last + !match e.len() { 0 => 1, _ => 2 }
}
