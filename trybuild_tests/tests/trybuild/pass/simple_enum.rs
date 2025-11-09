use butane::FieldType;

#[derive(FieldType)]
pub enum Status {
    Draft,
    Published,
    Archived,
}

fn main() {}
