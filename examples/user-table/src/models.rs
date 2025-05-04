//! Models for the user-table example.

use butane::{model, AutoPk, ForeignKey, Many};
use serde::{Deserialize, Serialize};

/// User metadata.
#[model]
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct User {
    /// User ID.
    pub id: String,
    /// User name.
    pub name: String,
    /// User email.
    pub email: String,
}
impl User {
    /// Create a new user.
    pub fn new(id: &str, name: &str, email: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            email: email.to_string(),
        }
    }
}

/// Post details, including a [ForeignKey] to [User].
#[model]
pub struct Post {
    /// Id of the blog post.
    pub id: AutoPk<i32>,
    /// Title of the blog post.
    pub title: String,
    /// Body of the blog post.
    pub body: String,
    /// Whether the blog post has been published.
    pub published: bool,
    /// Byline of the post.
    pub byline: Option<ForeignKey<User>>,
    /// Users who have liked the post.
    pub likes: Many<User>,
}
impl Post {
    /// Create a new Post.
    pub fn new(title: String, body: String) -> Self {
        Post {
            id: AutoPk::uninitialized(),
            title,
            body,
            published: false,
            byline: None,
            likes: Many::default(),
        }
    }
}
