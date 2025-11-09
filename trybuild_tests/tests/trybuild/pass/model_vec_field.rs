use butane::model;

// Vec<u8> is supported for BLOB data
#[model]
struct Post {
    #[pk]
    id: i64,
    tags: Vec<u8>,
}

fn main() {}
