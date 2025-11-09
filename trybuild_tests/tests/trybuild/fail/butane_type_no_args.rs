use butane::butane_type;

// This should fail - butane_type expects an argument
#[butane_type]
enum Status {
    Active,
    Inactive,
}

fn main() {}
