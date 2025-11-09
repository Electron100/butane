use butane::FieldType;
use serde::{Deserialize,Serialize};

#[derive(Deserialize, FieldType, Serialize)]
pub struct Metadata {
    pub title: String,
    pub version: i32,
}

fn main() {}
