# Butane Newtype Example

Demonstrates using `derive(FieldType)` for Butane model support on the newtype pattern.

To use this example, build the entire project using `cargo build` in the project root,
and then run these commands in this directory:

1. Initialise a Sqlite database using `cargo run -p butane_cli init sqlite db.sqlite`
2. Migrate the new sqlite database using `cargo run -p butane_cli migrate`
3. Run the commands, such as `cargo run --bin write_post_uuid`

