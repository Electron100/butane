use butane::db::{Connection, ConnectionAsync};
use butane::migrations::Migrations;
use butane::{find, find_async};
use butane_test_helper::*;
use butane_test_macros::butane_test;

use user_table::models::User;

#[maybe_async_cfg::maybe(
    sync(),
    async(),
    idents(
        Connection(sync = "Connection", async = "ConnectionAsync"),
        DataObjectOps(sync = "DataObjectOpsSync", async = "DataObjectOpsAsync"),
        find(sync = "find", async = "find_async"),
    )
)]
async fn insert_data(connection: &Connection) {
    use butane::DataObjectOps;

    let mut user = User::new("1", "Joe Bloggs", "bloggs@example.com");
    // TODO: This should fail, but it doesn't.
    user.save(connection).await.unwrap();

    // Check that the user was inserted correctly.
    // TODO: This fails on pg
    let user = find!(User, id == "1", connection).unwrap();
    assert_eq!(user.name, "Joe Bloggs");
}

#[butane_test(async, nomigrate)]
async fn migrate_and_unmigrate_async(mut connection: ConnectionAsync) {
    // Migrate forward.
    let base_dir = std::path::PathBuf::from(".butane");
    let migrations = butane_cli::get_migrations(&base_dir).unwrap();

    migrations.migrate_async(&mut connection).await.unwrap();

    insert_data_async(&connection).await;

    // Undo migrations.
    migrations.unmigrate_async(&mut connection).await.unwrap();
}

#[butane_test(sync, nomigrate)]
fn migrate_and_unmigrate_sync(mut connection: Connection) {
    // Migrate forward.
    let base_dir = std::path::PathBuf::from(".butane");
    let migrations = butane_cli::get_migrations(&base_dir).unwrap();

    migrations.migrate(&mut connection).unwrap();

    insert_data_sync(&connection);

    // Undo migrations.
    migrations.unmigrate(&mut connection).unwrap();
}
