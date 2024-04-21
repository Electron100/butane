//! Test the "current" migration created by the butane_test_helper due to
//! all of the other tests in the butane/tests directory.
#![cfg(test)]
use butane::db::Connection;
use butane::migrations::{Migration, Migrations};
use butane_test_helper::*;

fn unmigrate(mut connection: Connection) {
    let mem_migrations = create_current_migrations(&connection);

    let migrations = mem_migrations.unapplied_migrations(&connection).unwrap();
    assert_eq!(migrations.len(), 0);

    let migration = mem_migrations.latest().unwrap();
    migration.downgrade(&mut connection).unwrap();

    let migrations = mem_migrations.unapplied_migrations(&connection).unwrap();
    assert_eq!(migrations.len(), 1);
}
testall!(unmigrate);
