pub fn blank_lines(c: bool) -> u8 {
    let a = 1;

    let b = 2;
    if c {
        a
    } else if !c {
        b
    } else {
        // the last branch
        2
    }
}

pub fn after_blank() -> u8 {
    let x = "line one

line three";
    x.len() as u8
}

pub fn block_arms(o: Option<u8>, v: &mut Vec<u8>) -> u8 {
    match o {
        Some(_) => {}
        None => {}
    }
    match o {
        Some(x) => {
            v.push(x);
        }
        None => {
            v.clear();
        }
    }
    match o {
        Some(x) => v.push(x),
        None => {}
    };
    let n = match o {
        Some(x) => {
            let y = x + 1;
            y
        }
        None => 0,
    };
    n
}

pub fn one_line_statements(c: bool, v: &mut Vec<u8>) -> u8 {
    if c { v.push(1); }
    if c { v.push(2); } else { v.clear(); }
    if c { 1 } else { 2 };
    match c { true => 1, false => 2 };
    {
        v.push(3);
    }
    3
}

pub fn bounded<T>(t: T) -> T
where
    T: Clone, // {
{
    t.clone()
}

pub fn closing_comments(c: bool, v: &mut Vec<u8>) -> u8 {
    if c {
        v.push(1);
    } // pushed
    for x in [1, 2] {
        v.push(x);
    } /* both } */
    let n = if c {
        1
    } else {
        2
    }; // picked
    n
} // end

pub struct Attributed(u32);

impl Attributed {
    #[inline] pub fn get(&self) -> Vec<u32> {
        let v = vec![self.0];
        v
    }
    #[inline] #[must_use] pub fn n(&self) -> u32 { self.0 }
    #[allow(clippy::len_without_is_empty)] pub fn len(&self) -> usize {
        #[allow(unused)] let x = 1;
        self.0 as usize
    }
}
