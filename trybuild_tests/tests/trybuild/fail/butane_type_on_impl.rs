use butane::butane_type;

// This should fail - butane_type used on unsupported item (impl block)
#[butane_type(Text)]
impl SomeType {
    fn new() -> Self {
        Self
    }
}

struct SomeType;

fn main() {}
