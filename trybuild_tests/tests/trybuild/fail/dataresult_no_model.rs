use butane::dataresult;

// This should fail - dataresult without model argument
#[dataresult]
struct PostData {
    id: i64,
    title: String,
}

fn main() {}
