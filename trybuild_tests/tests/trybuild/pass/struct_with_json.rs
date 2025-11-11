use butane::FieldType;
use serde::{Deserialize,Serialize};

#[derive(Deserialize, FieldType, Serialize)]
struct Metadata {
    title: String,
    version: i32,
}

fn main() {}
