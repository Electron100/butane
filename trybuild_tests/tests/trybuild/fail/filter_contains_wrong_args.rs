use butane::{filter, model};

#[model]
struct Post {
    id: i64,
    title: String,
    tags: Vec<u8>,
}

fn main() {
    // This should fail - contains expects exactly one argument
    let _f = filter!(Post, tags.contains(1, 2));
}
