use butane::{filter, model};

#[model]
struct Post {
    id: i64,
    title: String,
}

fn main() {
    // This should fail - filter! expects both Type and expression
    let _f = filter!(Post);
}
