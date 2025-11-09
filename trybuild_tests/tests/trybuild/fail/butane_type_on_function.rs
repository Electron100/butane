use butane::butane_type;

// This should fail - butane_type on function
#[butane_type(Text)]
fn my_function() -> String {
    String::new()
}

fn main() {}
