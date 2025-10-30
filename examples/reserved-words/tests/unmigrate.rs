use butane::db::{Connection, ConnectionAsync};
use butane::migrations::Migrations;
use butane::{find, find_async};
use butane_test_helper::*;
use butane_test_macros::butane_test;

use reserved_words::models::{Post, RowidTest, User};

#[maybe_async_cfg::maybe(
    sync(),
    async(),
    idents(
        Connection(sync = "Connection", async = "ConnectionAsync"),
        DataObjectOps(sync = "DataObjectOpsSync", async = "DataObjectOpsAsync"),
        ForeignKeyOps(sync = "ForeignKeyOpsSync", async = "ForeignKeyOpsAsync"),
        ManyOps(sync = "ManyOpsSync", async = "ManyOpsAsync"),
        find(sync = "find", async = "find_async"),
    )
)]
async fn insert_data(connection: &Connection) {
    use butane::{DataObjectOps, ForeignKeyOps, ManyOps};

    if connection.backend_name() == "pg" {
        // This fails because User is a pg internal table.
        // See https://github.com/Electron100/butane/issues/334
        connection
            .execute("SELECT id, email from User")
            .await
            .unwrap_err();
    } else {
        connection
            .execute("SELECT id, email from User")
            .await
            .unwrap();
    }

    // This works because the table name is quoted.
    connection
        .execute("SELECT id, email from \"User\"")
        .await
        .unwrap();

    let mut user = User::new("1", "Joe Bloggs", "bloggs@example.com");
    user.save(connection).await.unwrap();

    let user = find!(User, id == "1", connection).unwrap();
    assert_eq!(user.name, "Joe Bloggs");

    let mut post = Post::new("Hello world", "This is a test");
    post.byline = Some(user.clone().into());
    post.save(connection).await.unwrap();
    post.likes.add(&user.clone()).unwrap();
    post.likes.save(connection).await.unwrap();

    let post = Post::get(connection, 1).await.unwrap();
    assert_eq!(post.title, "Hello world");
    assert_eq!(
        post.byline.unwrap().load(connection).await.unwrap().name,
        "Joe Bloggs"
    );
    assert_eq!(
        post.likes
            .load(connection)
            .await
            .unwrap()
            .into_iter()
            .count(),
        1
    );

    let mut rowid_test = RowidTest::new(5);
    rowid_test.save(connection).await.unwrap();
    RowidTest::get(connection, 5).await.unwrap();
}

#[test_log::test(butane_test(async, nomigrate, pg))]
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
