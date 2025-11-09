use butane::FieldType;
use serde::{Deserialize, Serialize};

// This should fail because we're using FieldType on a type that contains
// a field type that doesn't implement the required traits
#[derive(Deserialize, FieldType, Serialize)]
struct BadNewtype(std::marker::PhantomData<i32>);

fn main() {}
