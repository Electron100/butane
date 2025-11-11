use butane::{filter, model};

#[model]
struct Post {
    id: i64,
    title: String,
}

fn main() {
    // This should fail - unsupported filter expression (try/catch)
    let _f = filter!(Post, loop { break true; });
}
