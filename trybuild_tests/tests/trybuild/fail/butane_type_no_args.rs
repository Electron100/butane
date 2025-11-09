use butane::butane_type;

// This should fail - butane_type expects an argument
#[butane_type]
pub enum Status {
    Active,
    Inactive,
}

fn main() {}
