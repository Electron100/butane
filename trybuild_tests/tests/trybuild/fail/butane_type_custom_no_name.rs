use butane::butane_type;

// This should fail - Custom expects name in parens
#[butane_type(Custom)]
struct MyType {
    value: i32,
}

fn main() {}
