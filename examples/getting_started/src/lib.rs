//! Common helpers for the getting_started example CLI.

#![deny(missing_docs)]

pub mod butane_migrations;
pub mod models;

use butane::_filenames::{BUTANE_DIRNAME, CONNECTION_JSON_FILENAME};
use butane::db::{Connection, ConnectionSpec};
use butane::prelude::*;
use models::{Blog, Post};

/// Load a [Connection].
pub fn establish_connection() -> Connection {
    let connspec = format!("{BUTANE_DIRNAME}/{CONNECTION_JSON_FILENAME}");
    butane::db::connect(&ConnectionSpec::load(connspec).unwrap()).unwrap()
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
