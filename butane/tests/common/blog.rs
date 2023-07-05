use butane::prelude::*;
use butane::{dataresult, model};
use butane::{db::Connection, ForeignKey, Many, ObjectState};
use chrono::{naive::NaiveDateTime, offset::Utc};

#[cfg(feature = "fake")]
use fake::Dummy;

#[model]
#[derive(Debug, Eq, PartialEq)]
#[cfg_attr(feature = "fake", derive(Dummy))]
pub struct Blog {
    pub id: i64,
    pub name: String,
}
impl Blog {
    pub fn new(id: i64, name: &str) -> Self {
        Blog {
            id,
            name: name.to_string(),
            state: ObjectState::default(),
        }
    }
}

#[model]
#[derive(Debug, Eq, PartialEq)]
#[cfg_attr(feature = "fake", derive(Dummy))]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub published: bool,
    pub pub_time: std::option::Option<NaiveDateTime>,
    pub likes: i32,
    pub tags: Many<Tag>,
    pub blog: ForeignKey<Blog>,
}
impl Post {
    pub fn new(id: i64, title: &str, body: &str, blog: &Blog) -> Self {
        Post {
            id,
            title: title.to_string(),
            body: body.to_string(),
            published: false,
            pub_time: None,
            likes: 0,
            tags: Many::new(),
            blog: ForeignKey::from(blog),
            state: ObjectState::default(),
        }
    }
}

#[dataresult(Post)]
pub struct PostMetadata {
    pub id: i64,
    pub title: String,
    pub pub_time: Option<NaiveDateTime>,
}

#[model]
#[derive(Debug)]
#[cfg_attr(feature = "fake", derive(Dummy))]
#[table = "tags"]
pub struct Tag {
    #[pk]
    pub tag: String,
}
impl Tag {
    pub fn new(tag: &str) -> Self {
        Tag {
            tag: tag.to_string(),
            state: ObjectState::default(),
        }
    }
}

pub async fn create_tag(conn: &Connection, name: &str) -> Tag {
    let mut tag = Tag::new(name);
    tag.save(conn).await.unwrap();
    tag
}

/// Sets up two blogs
/// 1. "Cats"
/// 2. "Mountains"
#[allow(dead_code)] // only used by some test files
pub async fn setup_blog(conn: &Connection) {
    let mut cats_blog = Blog::new(1, "Cats");
    cats_blog.save(conn).await.unwrap();
    let mut mountains_blog = Blog::new(2, "Mountains");
    mountains_blog.save(conn).await.unwrap();

    let tag_asia = create_tag(conn, "asia").await;
    let tag_danger = create_tag(conn, "danger").await;

    let mut post = Post::new(
        1,
        "The Tiger",
        "The tiger is a cat which would very much like to eat you.",
        &cats_blog,
    );
    post.published = true;
    post.pub_time = Some(Utc::now().naive_utc());
    post.likes = 4;
    post.tags.add(&tag_danger).unwrap();
    post.tags.add(&tag_asia).unwrap();
    post.save(conn).await.unwrap();

    let mut post = Post::new(
        2,
        "Sir Charles",
        "Sir Charles (the Very Second) is a handsome orange gentleman",
        &cats_blog,
    );
    post.published = true;
    post.likes = 20;
    post.save(conn).await.unwrap();

    let mut post = Post::new(
        3,
        "Mount Doom",
        "You must throw the ring into Mount Doom. Then you get to ride on a cool eagle.",
        &mountains_blog,
    );
    post.published = true;
    post.likes = 10;
    post.tags.add(&tag_danger).unwrap();
    post.save(conn).await.unwrap();

    let mut post = Post::new(
        4,
        "Mt. Everest",
        "Everest has very little air, and lately it has very many people. This post is unfinished.",
        &mountains_blog,
    );
    post.published = false;
    post.tags.add(&tag_danger).unwrap();
    post.save(conn).await.unwrap();
}
