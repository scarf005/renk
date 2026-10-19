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
