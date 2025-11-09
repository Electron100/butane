use butane::model;

// This should fail - default value on primary key field
#[model]
struct Post {
    #[pk]
    #[default(0)]
    id: i64,
    title: String,
}

fn main() {}
