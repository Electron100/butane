use butane::db::{BackendConnection, Connection};
use butane::migrations::{Migration, Migrations};
use butane::DataObject;
use butane_test_helper::*;

use newtype::models::{Blog, Post, Tags};

fn insert_data(connection: &Connection) {
    if connection.backend_name() == "sqlite" {
        // https://github.com/Electron100/butane/issues/226
        return;
    }
    let mut cats_blog = Blog::new("Cats").unwrap();
    cats_blog.save(connection).unwrap();

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
    post.save(connection).unwrap();
}

fn migrate_and_rollback(mut connection: Connection) {
    // Migrate forward.
    let base_dir = std::path::PathBuf::from(".butane");
    let migrations = butane_cli::get_migrations(&base_dir).unwrap();
    let to_apply = migrations.unapplied_migrations(&connection).unwrap();
    for migration in &to_apply {
        migration
            .apply(&mut connection)
            .unwrap_or_else(|err| panic!("migration {} failed: {err}", migration.name()));
        eprintln!("Applied {}", migration.name());
    }

    insert_data(&connection);

    // Rollback migrations.
    for migration in to_apply.iter().rev() {
        if connection.backend_name() == "pg" && migration.name() == "20240401_095709389_init" {
            // Postgres error db error: ERROR: cannot drop table blog because other objects depend on it
            // DETAIL: constraint post_blog_fkey on table post depends on table blog
            // HINT: Use DROP ... CASCADE to drop the dependent objects too.
            let err = migration.downgrade(&mut connection).unwrap_err();
            eprintln!("Rolled back {} failed: {err:?}", migration.name());
            return;
        }

        migration
            .downgrade(&mut connection)
            .unwrap_or_else(|err| panic!("rollback of {} failed: {err}", migration.name()));
        eprintln!("Rolled back {}", migration.name());
    }
}
testall_no_migrate!(migrate_and_rollback);
