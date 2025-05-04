# Butane reserved table name example

Demonstrates that model "User" is stored in table "User" which does not conflict
with PostgreSQL reserved table name "user".

To use this example, build the entire project using `cargo build` in the project root,
and then run `cargo test` in this example.  See "tests/unmigrate.rs" to see how the
tests verify the model "User" can be used.
