use butane::model;

// This should fail - fields must be named (tuple struct)
#[model]
struct Post(i64, String, String);

fn main() {}
