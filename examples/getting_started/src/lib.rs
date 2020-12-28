pub mod models;

use models::{Blog, Post};
use propane::db::{Connection, ConnectionSpec};
use propane::prelude::*;

pub fn establish_connection() -> Connection {
    propane::db::connect(&ConnectionSpec::load("propane/connection.json").unwrap()).unwrap()
}

pub fn create_blog(conn: &Connection, name: impl Into<String>) -> Blog {
    let mut blog = Blog::new(name);
    blog.save(conn).expect("Error saving blog");
    blog
}

pub fn create_post(conn: &Connection, blog: &Blog, title: String, body: String) -> Post {
    let mut new_post = Post::new(blog, title, body);
    new_post.save(conn).expect("Error saving new post");
    new_post
}
