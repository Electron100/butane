use butane::FieldType;
use serde::{Deserialize, Serialize};

// This should fail - FieldType on a unit struct should produce an error
// Unit structs have no data and cannot be stored
#[derive(Deserialize, FieldType, Serialize)]
struct UnitStruct;

fn main() {}
