use butane::model;

// This should fail - default value with byte literal not supported
#[model]
struct Post {
    #[pk]
    id: i64,
    #[default(b'x')]
    code: u8,
}

fn main() {}
