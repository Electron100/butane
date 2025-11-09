use butane::FieldType;

// This should fail - deriving FieldType on empty enum
#[derive(FieldType)]
pub enum EmptyEnum {}

fn main() {}
