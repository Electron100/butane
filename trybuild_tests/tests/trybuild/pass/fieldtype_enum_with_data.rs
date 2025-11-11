use butane::FieldType;
use serde::{Deserialize, Serialize};

// This should pass - deriving FieldType on enum with data variants
// FieldType uses JSON serialization for complex enums with serde support
#[derive(Deserialize, FieldType, Serialize)]
enum Status {
    Active { since: i64 },
    Inactive,
}

fn main() {}
