use butane::model;

// This should fail - enum as model (models must be structs)
#[model]
enum Post {
    Draft { id: i64, title: String },
    Published { id: i64, title: String },
}

fn main() {}
