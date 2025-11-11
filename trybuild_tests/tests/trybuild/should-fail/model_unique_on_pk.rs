use butane::model;

// This should fail - unique attribute on pk field (redundant)
#[model]
struct Post {
    #[pk]
    #[unique]
    id: i64,
    title: String,
}

fn main() {}
