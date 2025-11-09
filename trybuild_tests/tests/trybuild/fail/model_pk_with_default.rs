use butane::model;

// This should fail - conflicting attributes on same field
#[model]
struct Post {
    #[pk]
    #[default(0)]
    id: i64,
    title: String,
}

fn main() {}
