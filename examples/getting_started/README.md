# butane `getting_started` example

This is the sync version of this example. There is also [an async version](../getting_started_async)

To use this example, build the entire project using `cargo build` in the project root,
and then run these commands in this directory:

1. Initialise a Sqlite database using `cargo run -p butane_cli init sqlite db.sqlite`
2. Migrate the new sqlite database using `cargo run -p butane_cli migrate`
3. Run the commands, such as `cargo run --bin write_post`

See [getting-started.md](https://github.com/Electron100/butane/blob/master/docs/getting-started.md)
for a detailed walkthrough of this example.
