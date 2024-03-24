use butane::db::Connection;
use butane::migrations::{Migration, Migrations};
use butane_test_helper::*;

fn migrate_and_rollback(mut connection: Connection) {
    // Migrate forward.
    let base_dir = std::path::PathBuf::from(".butane");
    let migrations = butane_cli::get_migrations(&base_dir).unwrap();
    let to_apply = migrations.unapplied_migrations(&connection).unwrap();
    for migration in &to_apply {
        migration.apply(&mut connection).unwrap();
        eprintln!("Applied {}", migration.name());
    }

    // TODO: Insert data

    // Rollback migrations.
    for migration in to_apply.iter().rev() {
        migration.downgrade(&mut connection).unwrap();
        eprintln!("Rolled back {}", migration.name());
    }
}
testall_no_migrate!(migrate_and_rollback);
