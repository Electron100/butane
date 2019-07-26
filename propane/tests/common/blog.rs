use propane::model;
use propane::prelude::*;
use propane::{db::Connection, ForeignKey};

#[model]
#[derive(Debug, Eq, PartialEq)]
pub struct Blog {
    pub id: i64,
    pub name: String,
}
impl Blog {
    pub fn new(id: i64, name: &str) -> Self {
        Blog {
            id,
            name: name.to_string(),
        }
    }
}

#[model]
#[derive(Debug, Eq, PartialEq)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub published: bool,
    pub likes: i32,
    // TODO support ManyToMany
    //pub tags: ManyToMany<Tag>,
    pub blog: ForeignKey<Blog>,
}
impl Post {
    pub fn new(id: i64, title: &str, body: &str, blog: &Blog) -> Self {
        Post {
            id,
            title: title.to_string(),
            body: body.to_string(),
            published: false,
            likes: 0,
            blog: ForeignKey::from(blog),
        }
    }
}

#[model]
struct Tag {
    #[pk]
    tag: String,
}

/// Sets up two blogs
/// 1. "Cats"
/// 2. "Mountains"
#[allow(dead_code)] // only used by some test files
pub fn setup_blog(conn: &Connection) {
    let mut cats_blog = Blog::new(1, "Cats");
    cats_blog.save(conn).unwrap();
    let mut mountains_blog = Blog::new(2, "Mountains");
    mountains_blog.save(conn).unwrap();

    let mut post = Post::new(
        1,
        "The Tiger",
        "The tiger is a cat which would very much like to eat you.",
        &cats_blog,
    );
    post.published = true;
    post.likes = 4;
    post.save(conn).unwrap();

    let mut post = Post::new(
        2,
        "Sir Charles",
        "Sir Charles (the Very Second) is a handsome orange gentleman",
        &cats_blog,
    );
    post.published = true;
    post.likes = 20;
    post.save(conn).unwrap();

    let mut post = Post::new(
        3,
        "Mount Doom",
        "You must throw the ring into Mount Doom. Then you get to ride on a cool eagle.",
        &mountains_blog,
    );
    post.published = true;
    post.likes = 10;
    post.save(conn).unwrap();

    let mut post = Post::new(
        4,
        "Mt. Everest",
        "Everest has very little air, and lately it has very many people. This post is unfinished.",
        &mountains_blog,
    );
    post.published = false;
    post.save(conn).unwrap();
}
