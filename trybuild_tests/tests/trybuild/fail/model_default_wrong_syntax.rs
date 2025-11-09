use butane::model;

// This should fail - default with wrong syntax (not key = value)
#[model]
struct Post {
    #[pk]
    id: i64,
    #[default("test")]
    title: String,
}

fn main() {}
