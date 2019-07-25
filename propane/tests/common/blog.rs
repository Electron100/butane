use propane::model;
use propane::prelude::*;
use propane::{db::Connection, ForeignKey};

#[model]
pub struct Blog {
    id: i64,
    name: String,
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
pub struct Post {
    id: i64,
    title: String,
    body: String,
    published: bool,
    likes: i32,
    // TODO support ManyToMany
    //tags: ManyToMany<Tag>,
    blog: ForeignKey<Blog>,
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
pub fn setup_blog(conn: &Connection) {
    let mut cats_blog = Blog::new(1, "Cats");
    cats_blog.save(conn).unwrap();
    let mut mountains_blog = Blog::new(1, "Mountains");
    mountains_blog.save(conn).unwrap();

    let mut post = Post::new(
        1,
        "The Tiger",
        "The tiger is a cat which would very much like to eat you.",
        &cats_blog,
    );
    post.published = true;
    post.likes = 2;
    post.save(conn).unwrap();

    let mut post = Post::new(
        2,
        "Sir Charles",
        "Sir Charles (the Very Second) is a handsome orange gentleman",
        &cats_blog,
    );
    post.published = true;
    post.likes = 3;
    post.save(conn).unwrap();

    let mut post = Post::new(
        3,
        "Mount Doom",
        "Mount Doom is where you must throw evil rings",
        &mountains_blog,
    );
    post.published = true;
    post.likes = 4;
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
