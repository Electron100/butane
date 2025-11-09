use butane::butane_type;

// This should fail - no SqlType value with this name
#[butane_type(NotARealType)]
pub type UserId = i32;

fn main() {}
