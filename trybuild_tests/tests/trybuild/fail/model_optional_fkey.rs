use butane::model;

// This should fail - Option<ForeignKey> is not supported
#[model]
struct Post {
    #[pk]
    id: i64,
    blog: Option<butane::ForeignKey<Blog>>,
}

struct Blog {
    id: i64,
}

fn main() {}
