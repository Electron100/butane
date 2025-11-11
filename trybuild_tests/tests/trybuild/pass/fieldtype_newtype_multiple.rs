use butane::FieldType;
use serde::{Deserialize, Serialize};

// This should fail - newtype with multiple fields
#[derive(Deserialize, FieldType, Serialize)]
struct MultiField(i32, String);

fn main() {}
