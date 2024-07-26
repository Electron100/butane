//! Test the "current" migration created by the butane_test_helper due to
//! all of the other tests in the butane/tests directory.
#![cfg(test)]
use butane::db::Connection;
use butane::migrations::{Migration, Migrations};
use butane_test_helper::*;

async fn unmigrate(mut connection: Connection) {
    let mem_migrations = create_current_migrations(&connection);

    connection
        .with_sync(move |conn| {
            let migrations = mem_migrations.unapplied_migrations(conn).unwrap();
            assert_eq!(migrations.len(), 0);

            let migration = mem_migrations.latest().unwrap();
            migration.downgrade(conn).unwrap();

            let migrations = mem_migrations.unapplied_migrations(conn).unwrap();
            assert_eq!(migrations.len(), 1);
            Ok(())
        })
        .await
        .unwrap();
}
testall!(unmigrate);
