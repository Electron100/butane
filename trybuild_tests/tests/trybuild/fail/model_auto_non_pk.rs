use butane::{model, AutoPk};

// This should fail - can't use AutoPk on non-primary key field.
// TODO: It also emits a strange/confusing compiler error.
#[model]
struct Post {
    #[pk]
    id: i64,
    version: AutoPk<i32>,
}

fn main() {}
