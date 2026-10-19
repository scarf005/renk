pub fn classify(values: &[u32], limit: u32) -> &'static str {
    let mut index = 0;
    while index < values.len()
        && values[index] < limit
    {
        index += 1;
    }
    if values.is_empty()
        || values.iter().all(|value| *value == 0)
    {
        "empty"
    } else if values.len() > 3
        && values[0] > limit // first
    { // many
        "many"
    } else {
        "few"
    }
}

pub struct Scanner {
    pub text: String,
}

impl Scanner {
    pub fn braces(&self) -> usize {
        let pattern = "
{
";
        match self
            .text
            .matches(pattern)
            .count()
        {
            0 => 0,
            count => count,
        }
    }
}
