use butane::{filter, model};

#[model]
struct Post {
    id: i64,
    title: String,
}

fn main() {
    // This should fail - unknown method call
    let _f = filter!(Post, title.starts_with("test"));
}
