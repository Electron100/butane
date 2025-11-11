use butane::model;

// This should fail - malformed default value (not a literal)
#[model]
struct Post {
    #[pk]
    id: i64,
    #[default(get_default())]
    title: String,
}

fn main() {}
