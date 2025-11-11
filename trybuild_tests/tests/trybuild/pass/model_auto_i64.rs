use butane::{model, AutoPk};

// This should pass - AutoPk on i64 primary key
#[model]
struct Post {
    #[pk]
    id: AutoPk<i64>,
    title: String,
}

fn main() {}
