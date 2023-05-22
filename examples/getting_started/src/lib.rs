pub mod models;

use butane::db::{Connection, ConnectionSpec};
use butane::prelude::*;
use models::{Blog, Post};

pub async fn establish_connection() -> Connection {
    butane::db::connect(&ConnectionSpec::load(".butane/connection.json").unwrap())
        .await
        .unwrap()
}

pub async fn create_blog(conn: &Connection, name: impl Into<String>) -> Blog {
    let mut blog = Blog::new(name);
    blog.save(conn).await.unwrap();
    blog
}

pub async fn create_post(conn: &Connection, blog: &Blog, title: String, body: String) -> Post {
    let mut new_post = Post::new(blog, title, body);
    new_post.save(conn).await.unwrap();
    new_post
}

pub async fn existing_blog(conn: &Connection) -> Option<Blog> {
    Blog::query().load_first(conn).await.unwrap()
}
