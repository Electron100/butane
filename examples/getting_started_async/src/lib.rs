//! Common helpers for the getting_started example CLI.

#![deny(missing_docs)]

pub mod butane_migrations;
pub mod models;

use butane::db::{ConnectionAsync, ConnectionSpec};
use butane::migrations::Migrations;
use butane::prelude_async::*;
use models::{Blog, Post};

/// Load a [Connection].
pub async fn establish_connection() -> ConnectionAsync {
    let mut connection =
        butane::db::connect_async(&ConnectionSpec::load(".butane/connection.json").unwrap())
            .await
            .unwrap();
    let migrations = butane_migrations::get_migrations().unwrap();
    migrations.migrate_async(&mut connection).await.unwrap();
    connection
}

/// Create a [Blog].
pub async fn create_blog(conn: &ConnectionAsync, name: impl Into<String>) -> Blog {
    let mut blog = Blog::new(name);
    blog.save(conn).await.unwrap();
    blog
}

/// Create a [Post].
pub async fn create_post(conn: &ConnectionAsync, blog: &Blog, title: String, body: String) -> Post {
    let mut new_post = Post::new(blog, title, body);
    new_post.save(conn).await.unwrap();
    new_post
}

/// Fetch the first existing [Blog] if one exists.
pub async fn existing_blog(conn: &ConnectionAsync) -> Option<Blog> {
    Blog::query().load_first(conn).await.unwrap()
}
