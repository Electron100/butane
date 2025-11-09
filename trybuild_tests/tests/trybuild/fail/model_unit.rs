use butane::model;

// This should fail - model on a unit struct should produce an error
// Models need fields, including at least a primary key
#[model]
struct UnitModel;

fn main() {}
