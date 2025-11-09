use butane::model;

// This should fail - no pk field found
#[model]
struct Post {
    title: String,
    content: String,
}

fn main() {}
