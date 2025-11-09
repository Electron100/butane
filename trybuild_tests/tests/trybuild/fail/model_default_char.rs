use butane::model;

// This should fail - default value with char literal not supported
#[model]
struct Post {
    #[pk]
    id: i64,
    #[default('x')]
    initial: char,
}

fn main() {}
