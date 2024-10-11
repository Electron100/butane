use butane::db::{BackendConnection, Connection};
use butane::migrations::Migrations;
use butane::DataObjectOpAsync;
use butane_test_helper::*;
use butane_test_macros::butane_test;

use getting_started::models::{Blog, Post, Tag};

async fn create_tag(connection: &Connection, name: &str) -> Tag {
    let mut tag = Tag::new(name);
    tag.save(connection).await.unwrap();
    tag
}

async fn insert_data(connection: &Connection) {
    if connection.backend_name() == "sqlite" {
        // https://github.com/Electron100/butane/issues/226
        return;
    }
    let mut cats_blog = Blog::new("Cats");
    cats_blog.save(connection).await.unwrap();

    let tag_asia = create_tag(connection, "asia").await;
    let tag_danger = create_tag(connection, "danger").await;

    let mut post = Post::new(
        &cats_blog,
        "The Tiger".to_string(),
        "The tiger is a cat which would very much like to eat you.".to_string(),
    );
    post.published = true;
    post.likes = 4;
    post.tags.add(&tag_danger).unwrap();
    post.tags.add(&tag_asia).unwrap();
    post.save(connection).await.unwrap();
}

#[butane_test]
async fn migrate_and_unmigrate(mut connection: Connection) {
    // Migrate forward.
    let base_dir = std::path::PathBuf::from(".butane");
    let migrations = butane_cli::get_migrations(&base_dir).unwrap();

    migrations.migrate(&mut connection).await.unwrap();

    insert_data(&connection).await;

    // Undo migrations.
    migrations.unmigrate(&mut connection).await.unwrap();
}
