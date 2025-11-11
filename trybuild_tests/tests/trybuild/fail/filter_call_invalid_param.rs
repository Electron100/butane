use butane::{filter, model};

#[model]
struct Post {
    id: i64,
    title: String,
}

fn main() {
    // This should fail - like does not accept call expressions
    let _f = filter!(Post, title.like(some_function()));
}
