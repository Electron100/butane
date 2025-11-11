use butane::{model, AutoPk};

// This should fail - AutoPk only supports integer types
#[model]
struct Post {
    #[pk]
    id: AutoPk<String>,
    title: String,
}

fn main() {}
