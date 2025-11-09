use butane::FieldType;

use butane::FieldType;
use serde::{Deserialize, Serialize};

// This should fail - FieldType on a union should produce an error
// Unions cannot be properly serialized/deserialized
#[derive(FieldType, Serialize, Deserialize)]
pub union MyUnion {
    int_val: i32,
    float_val: f32,
}

fn main() {}
