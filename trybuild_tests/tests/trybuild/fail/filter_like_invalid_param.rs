use butane::{filter, model};

#[model]
struct Post {
    id: i64,
    title: String,
}

fn main() {
    // This should fail - like expects a string literal, not a binary expression
    let _f = filter!(Post, title.like(1 + 2));
}
