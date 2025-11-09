use butane::FieldType;
use serde::{Deserialize, Serialize};

// This should fail - deriving FieldType on tuple struct with named fields
#[derive(Deserialize, FieldType, Serialize)]
struct Hybrid(i32, name: String);

fn main() {}
