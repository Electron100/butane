use butane::model;

// This should fail - multiple pk fields
#[model]
struct Post {
    #[pk]
    id: i64,
    #[pk]
    uuid: String,
    title: String,
}

fn main() {}
