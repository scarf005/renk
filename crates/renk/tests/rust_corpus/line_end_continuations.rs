pub struct Stream {
    pub limit: u32,
}

impl Stream {
    pub fn force_wake(&self, need: u32) -> bool {
        let wake =
            // the stream needs a poll
            need & 1 != 0 && self.limit > 0;
        let URGENT =
            /* a block comment */
            need > 8;
        let text = "ends with =";
        wake || URGENT || text.is_empty()
    }

    pub fn plus_one(&self, values: Vec<u32>) -> Vec<u32> {
        values
            .into_iter()
            .map(|v|
                // SAFETY: nothing unsafe happens here
                v + 1)
            .map(|v| // keep the value
                v)
            .map(move |v|
                v + self.limit)
            .filter(|v| v % 2 == 0 ||
                v % 3 == 0)
            .collect()
    }

    pub fn spawn(&self) -> u32 {
        let add = |a: u32, b: u32|
            a + b;
        let bits = [self.limit |
            1];
        std::thread::spawn(||
            7).join().unwrap() + add(bits[0], 1)
    }
}
