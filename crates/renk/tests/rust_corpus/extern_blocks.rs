unsafe extern "C" {
    pub fn abs(value: i32) -> i32;
    pub safe fn labs(value: i64) -> i64;
    pub static environ: *const *const u8;
}

#[link(name = "m")]
unsafe extern "C" {
    fn cos(value: f64) -> f64;
}

#[allow(unused)] unsafe extern "C" {
    // }
    fn sin(value: f64) -> f64; /* { */
}

unsafe extern "C" { fn tan(value: f64) -> f64; }

unsafe extern "C" {}

unsafe extern "C" {
}

pub unsafe extern "C" fn callback(value: i32) -> i32 {
    value
}

pub fn cosine(value: f64) -> f64 {
    unsafe { cos(value) + sin(value) + tan(value) }
}
