use butane::{model, AutoPk};

// This should fail - AutoPk without generic parameter
#[model]
struct Post {
    #[pk]
    id: AutoPk,
    title: String,
}

fn main() {}
