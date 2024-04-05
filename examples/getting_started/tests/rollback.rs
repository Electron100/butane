use butane::db::{BackendConnection, Connection};
use butane::migrations::{Migration, Migrations};
use butane::DataObject;
use butane_test_helper::*;

use getting_started::models::{Blog, Post, Tag};

fn create_tag(connection: &Connection, name: &str) -> Tag {
    let mut tag = Tag::new(name);
    tag.save(connection).unwrap();
    tag
}

fn insert_data(connection: &Connection) {
    if connection.backend_name() == "sqlite" {
        // https://github.com/Electron100/butane/issues/226
        return;
    }
    let mut cats_blog = Blog::new("Cats");
    cats_blog.save(connection).unwrap();

    let tag_asia = create_tag(connection, "asia");
    let tag_danger = create_tag(connection, "danger");

    let mut post = Post::new(
        &cats_blog,
        "The Tiger".to_string(),
        "The tiger is a cat which would very much like to eat you.".to_string(),
    );
    post.published = true;
    post.likes = 4;
    post.tags.add(&tag_danger).unwrap();
    post.tags.add(&tag_asia).unwrap();
    post.save(connection).unwrap();
}

fn migrate_and_rollback(mut connection: Connection) {
    // Migrate forward.
    let base_dir = std::path::PathBuf::from(".butane");
    let migrations = butane_cli::get_migrations(&base_dir).unwrap();
    let to_apply = migrations.unapplied_migrations(&connection).unwrap();
    for migration in &to_apply {
        if connection.backend_name() == "pg"
            && migration.name() == "20240115_023841384_dbconstraints"
        {
            // migration 20240115_023841384_dbconstraints failed: Postgres error db error:
            // ERROR: cannot drop table tag because other objects depend on it
            // DETAIL: constraint post_tags_many__butane_tmp_has_fkey1 on table post_tags_many depends on table tag
            let err = migration.apply(&mut connection).unwrap_err();
            eprintln!("Migration {} failed: {err:?}", migration.name());
            return;
        }
        migration
            .apply(&mut connection)
            .unwrap_or_else(|err| panic!("migration {} failed: {err}", migration.name()));
        eprintln!("Applied {}", migration.name());
    }

    insert_data(&connection);

    // Rollback migrations.
    for migration in to_apply.iter().rev() {
        migration
            .downgrade(&mut connection)
            .unwrap_or_else(|err| panic!("rollback of {} failed: {err}", migration.name()));
        eprintln!("Rolled back {}", migration.name());
    }
}
testall_no_migrate!(migrate_and_rollback);
