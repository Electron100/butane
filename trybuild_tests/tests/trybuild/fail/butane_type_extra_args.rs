use butane::butane_type;

// This should fail - unexpected tokens in butane_type
#[butane_type(Text, Extra)]
pub enum Status {
    Active,
    Inactive,
}

fn main() {}
