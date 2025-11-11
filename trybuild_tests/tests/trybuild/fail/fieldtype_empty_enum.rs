use butane::FieldType;

// This should fail - deriving FieldType on empty enum
#[derive(FieldType)]
enum EmptyEnum {}

fn main() {}
