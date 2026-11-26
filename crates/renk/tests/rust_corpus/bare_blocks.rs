pub fn blocks(flag: bool) -> u8 {
    let ret;
    // the place we are taking from
    {
        ret = if flag { 1 } else { 2 };
    }
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    {
        if ret > 1 {
            return ret;
        }
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
    {
        let _ = ret;
    }
    {
        let doubled = ret * 2; // {
        doubled
    }
}

pub fn keywords_inside_chains(v: &[u32]) -> usize {
    let n = v
        .iter()
        .filter(|x| if **x > 1 { true } else { false })
        .count();
    {
        println!("{n}");
    }
    n
}

pub fn cfg_tail_blocks(n: u8) -> u8 {
    #[cfg(unix)]
    {
        u8::max(
            n,
            1,
        )
    }
    #[cfg(not(unix))]
    {
        let doubled = n * 2;
        doubled
    }
}

pub fn cfg_tail_mixed(n: u8) -> u8 {
    #[cfg(unix)]
    { n }
    #[cfg(not(unix))]
    { n + 1 }
}
