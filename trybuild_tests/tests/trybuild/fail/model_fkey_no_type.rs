use butane::{model, ForeignKey};

// This should fail - ForeignKey without type argument
#[model]
struct Post {
    #[pk]
    id: i64,
    blog: ForeignKey,
}

fn main() {}
