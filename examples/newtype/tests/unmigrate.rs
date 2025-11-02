use butane::db::{BackendConnectionAsync, Connection, ConnectionAsync};
use butane::migrations::Migrations;
use butane_test_helper::*;
use butane_test_macros::butane_test;

use newtype::models::{Blog, Post, Tags};

#[maybe_async_cfg::maybe(
    sync(),
    async(),
    idents(
        Connection(sync = "Connection", async = "ConnectionAsync"),
        DataObjectOps(sync = "DataObjectOpsSync", async = "DataObjectOpsAsync")
    )
)]
async fn insert_data(connection: &Connection) {
    use butane::DataObjectOps;
    // Turso: Skip due to table rename migration issue
    // See docs/turso-backend.md - Known Issues - Table Rename Migration
    if connection.backend_name() == "turso" || connection.backend_name() == "libsql" {
        return;
    }
    let mut cats_blog = Blog::new("Cats").unwrap();
    cats_blog.save(connection).await.unwrap();

    let mut post = Post::new(
        &cats_blog,
        "The Tiger".to_string(),
        "The tiger is a cat which would very much like to eat you.".to_string(),
    );
    post.published = true;
    post.likes = 4;
    post.tags = Tags(std::collections::HashSet::from([
        "asia".to_string(),
        "danger".to_string(),
    ]));
    post.save(connection).await.unwrap();
}

#[butane_test(async, nomigrate)]
async fn migrate_and_unmigrate_async(mut connection: ConnectionAsync) {
    // Migrate forward.
    let base_dir = std::path::PathBuf::from(".butane");
    let migrations = butane_cli::get_migrations(&base_dir).unwrap();

    // Turso: Skip unmigrate test due to table rename limitation
    // See docs/turso-backend.md - Known Issues - Table Rename Migration
    if connection.backend_name() == "turso" {
        return;
    }

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
