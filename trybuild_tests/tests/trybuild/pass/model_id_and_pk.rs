use butane::model;

// This should pass - field named 'id' with #[pk] on another field is allowed
// The #[pk] attribute takes precedence over the 'id' field name convention
#[model]
struct Post {
    id: String,
    #[pk]
    post_id: i64,
    title: String,
}

fn main() {}
