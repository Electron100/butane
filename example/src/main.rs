use failure;
use propane::db::{Connection, ConnectionSpec};
use propane::model;
use propane::{find, query};
use propane::{ForeignKey, Many};

use propane::prelude::*;

pub type Result<T> = std::result::Result<T, failure::Error>;

#[model]
struct Blog {
    id: i64,
    name: String,
}

#[model]
struct Post {
    id: i64,
    title: String,
    body: String,
    published: bool,
    likes: i32,
    tags: Many<Tag>,
    blog: ForeignKey<Blog>,
}

#[model]
struct Tag {
    #[pk]
    tag: String,
}

fn query() -> Result<()> {
    let conn = establish_connection()?;
    let _specific_post = Post::get(&conn, 1);
    let published_posts = query!(Post, published == true).limit(5).load(&conn)?;
    let unliked_posts = query!(Post, published == true && likes < 5).load(&conn)?;
    let blog: &Blog = unliked_posts.first().unwrap().blog.load(&conn)?;
    let tagged_posts = query!(Post, tags.contains("dinosaurs")).load(&conn);
    //let tagged_posts2 = query!(Post, tags.contains(tag = "dinosaurs")).load(&conn);
    let blog: Blog = find!(Blog, name == "Bears", &conn).unwrap();
    let posts_in_blog = query!(Post, blog == { &blog }).load(&conn);
    let posts_in_blog2 = query!(Post, blog == { blog }).load(&conn);
    let posts_in_blog = query!(Post, blog.matches(name == "Bears")).load(&conn);
    Ok(())
}

fn establish_connection() -> Result<Connection> {
    let spec = ConnectionSpec::load(&std::env::current_dir()?)?;
    let conn = propane::db::connect(&spec)?;
    Ok(conn)
}
fn main() {
    println!("Hello, world!");
}
