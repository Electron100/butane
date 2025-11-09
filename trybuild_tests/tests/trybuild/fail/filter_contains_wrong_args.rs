use butane::{filter, model};

#[model]
struct Post {
    id: i64,
    title: String,
    tags: Vec<String>,
}

fn main() {
    // This should fail - contains expects exactly one argument
    let _f = filter!(Post, tags.contains("a", "b"));
}
