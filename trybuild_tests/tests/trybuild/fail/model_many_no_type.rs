use butane::{model, Many};

// This should fail - Many without type argument
#[model]
struct Post {
    #[pk]
    id: i64,
    tags: Many,
}

fn main() {}
