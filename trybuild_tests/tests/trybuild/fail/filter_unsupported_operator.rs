use butane::{filter, model};

#[model]
struct Post {
    id: i64,
    title: String,
}

fn main() {
    // This should fail - unsupported binary operator
    let _f = filter!(Post, title ^ "test");
}
