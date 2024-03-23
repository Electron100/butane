//! Common helpers for the getting_started example CLI.

#![deny(missing_docs)]

pub mod butane_migrations;
pub mod models;

use butane::db::{Connection, ConnectionSpec};
use butane::migrations::{Migration, Migrations};
use butane::prelude::*;
use models::{Blog, Post};

/// Load a [Connection].
pub fn establish_connection() -> Connection {
    let mut connection =
        butane::db::connect(&ConnectionSpec::load(".butane/connection.json").unwrap()).unwrap();
    let migrations = butane_migrations::get_migrations().unwrap();
    let to_apply = migrations.unapplied_migrations(&connection).unwrap();
    for migration in to_apply {
        migration.apply(&mut connection).unwrap();
    }
    connection
}

/// Create a [Blog].
pub fn create_blog(conn: &Connection, name: impl Into<String>) -> Blog {
    let mut blog = Blog::new(name);
    blog.save(conn).unwrap();
    blog
}

/// Create a [Post].
pub fn create_post(conn: &Connection, blog: &Blog, title: String, body: String) -> Post {
    let mut new_post = Post::new(blog, title, body);
    new_post.save(conn).unwrap();
    new_post
}

/// Fetch the first existing [Blog] if one exists.
pub fn existing_blog(conn: &Connection) -> Option<Blog> {
    Blog::query().load_first(conn).unwrap()
}
