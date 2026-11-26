pub enum Message {
    Move {
        x: i32,
        y: i32,
    },
    Write(String),
    Quit,
}

pub fn score(message: Message) -> i32 {
    match message {
        Message::Move { x, y } => x + y,
        Message::Write(text) => text.len() as i32,
        Message::Quit => 0,
    }
}

pub fn is_keyword(word: &str, flags: u8) -> bool {
    let mask = flags |
        1;
    let pick = |bit: u8| bit & mask == bit;
    let either = pick(1) ||
        pick(2);
    match word {
        "_" |
        // Reserved words.
        "as" | "break" |

        "const" | "continue" => either,
        "fn" |
        "if" => pick(4),
        _ => false,
    }
}
