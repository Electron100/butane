use butane::db::{BackendConnection, Connection};
use butane::migrations::Migrations;
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

fn migrate_and_unmigrate(mut connection: Connection) {
    // Migrate forward.
    let base_dir = std::path::PathBuf::from(".butane");
    let migrations = butane_cli::get_migrations(&base_dir).unwrap();

    migrations.migrate(&mut connection).unwrap();

    insert_data(&connection);

    // Undo migrations.
    migrations.unmigrate(&mut connection).unwrap();
}
testall_no_migrate!(migrate_and_unmigrate);
