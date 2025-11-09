use butane::butane_type;

// This should fail - unexpected tokens in butane_type
#[butane_type(Text, Extra)]
enum Status {
    Active,
    Inactive,
}

fn main() {}
