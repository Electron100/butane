use butane::dataresult;

// This should fail - fields must be named for dataresult
#[dataresult(Post)]
struct PostData(i64, String);

fn main() {}
