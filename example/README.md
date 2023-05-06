# butane example

To use this example, build the entire project using `cargo build` in the project root,
and then run these commands in this directory:

1. Initialise a Sqlite database using `cargo run -p butane_cli init sqlite db.sqlite`
2. Initialise the migrations using `cargo run -p butane_cli makemigration initial`
3. Migrate the new sqlite database using `cargo run -p butane_cli migrate`
4. Run the example `../target/debug/example`

Any use of `cargo` to build/run this project will likely delete &
recreate the `example/.butane` directory, and the above steps will
need to be repeated.
