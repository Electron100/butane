use butane::prelude::*;
use butane::{model, ForeignKey, Many, ObjectState};

#[model]
#[derive(Clone, Debug, Default)]
pub struct Blog {
    #[auto]
    pub id: i64,
    pub name: String,
}
impl Blog {
    pub fn new(name: impl Into<String>) -> Self {
        Blog {
            name: name.into(),
            ..Default::default()
        }
    }
}

#[model]
#[derive(Clone, Debug)]
pub struct Post {
    #[auto]
    pub id: i32,
    pub title: String,
    pub body: String,
    pub published: bool,
    pub tags: Many<Tag>,
    pub blog: ForeignKey<Blog>,
    pub byline: Option<String>,
    pub likes: i32,
    state: butane::ObjectState,
}
impl Post {
    pub fn new(blog: &Blog, title: String, body: String) -> Self {
        Post {
            id: -1,
            title,
            body,
            published: false,
            tags: Many::default(),
            blog: blog.into(),
            byline: None,
            likes: 0,
            state: ObjectState::default(),
        }
    }
}

#[model]
#[derive(Clone, Debug, Default)]
pub struct Tag {
    #[pk]
    pub tag: String,
}
impl Tag {
    pub fn new(tag: impl Into<String>) -> Self {
        Tag {
            tag: tag.into(),
            ..Default::default()
        }
    }
}
