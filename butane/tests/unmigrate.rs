//! Test the "current" migration created by the butane_test_helper due to
//! all of the other tests in the butane/tests directory.
#![cfg(test)]
use butane::db::{Connection, ConnectionAsync};
use butane::migrations::{Migration, Migrations};
use butane_test_helper::*;
use butane_test_macros::*;

#[butane_test(async)]
async fn unmigrate_async(mut connection: ConnectionAsync) {
    let mem_migrations = create_current_migrations(connection.backend());

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

#[butane_test(sync)]
fn unmigrate_sync(mut conn: Connection) {
    let mem_migrations = create_current_migrations(conn.backend());

    let migrations = mem_migrations.unapplied_migrations(&conn).unwrap();
    assert_eq!(migrations.len(), 0);

    let migration = mem_migrations.latest().unwrap();
    migration.downgrade(&mut conn).unwrap();

    let migrations = mem_migrations.unapplied_migrations(&conn).unwrap();
    assert_eq!(migrations.len(), 1);
}
