use butane::model;

// This should pass - private fields are allowed in models
#[model]
pub struct Post {
    #[pk]
    pub id: i64,
    title: String,
}

fn main() {}
