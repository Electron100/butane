use butane::model;

// This should fail - Vec of non-Many type
#[model]
struct Post {
    #[pk]
    id: i64,
    tags: Vec<String>,
}

fn main() {}
