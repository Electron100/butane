pub mod models;

use models::Post;
use propane;
use propane::db::{Connection, ConnectionSpec};
use propane::prelude::*;

pub fn establish_connection() -> Connection {
    propane::db::connect(&ConnectionSpec::load("propane/connection.json").unwrap()).unwrap()
}

pub fn create_post(conn: &Connection, title: String, body: String) -> Post {
    let mut new_post = Post::new(title, body);
    new_post.save(conn).expect("Error saving new post");
    new_post
}
