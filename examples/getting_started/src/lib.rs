//! Common helpers for the getting_started example CLI.

pub mod models;

use butane::db::{Connection, ConnectionSpec};
use butane::prelude::*;
use models::{Blog, Post};

/// Load a [Connection].
pub fn establish_connection() -> Connection {
    butane::db::connect(&ConnectionSpec::load(".butane/connection.json").unwrap()).unwrap()
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
