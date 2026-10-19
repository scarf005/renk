/*!
Crate docs in a block comment.
*/

/**
Item docs in a block comment.
*/
pub fn documented(value: u8) -> u8 {
    /*
    A block comment as a statement.
    */
    let doubled = value * 2; /* trailing */
    /* one line */
    doubled
}

/* between items */
pub struct Unit;
