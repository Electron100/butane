use butane::{model, AutoPk};

// This should fail - AutoPk with wrong generic type (non-integer)
#[model]
struct Post {
    #[pk]
    id: AutoPk<f64>,
    title: String,
}

fn main() {}
