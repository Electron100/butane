//! Models for the user-table example.

use butane::{model, AutoPk, ForeignKey, Many};
use serde::{Deserialize, Serialize};

/// User metadata.
#[model]
#[derive(Debug, Default, Deserialize, Clone, Serialize)]
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
#[derive(Debug, Default, Deserialize, Clone, Serialize)]
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
    pub fn new(title: &str, body: &str) -> Self {
        Post {
            id: AutoPk::uninitialized(),
            title: title.to_string(),
            body: body.to_string(),
            published: false,
            byline: None,
            likes: Many::default(),
        }
    }
}

/// Model which uses the SQLite reserved word `rowid` as a column name.
#[model]
pub struct RowidTest {
    /// Primary key which uses the sqlite keyword `rowid`.
    #[pk]
    pub rowid: i32,
}
impl RowidTest {
    /// Create a new `sqlite_schema` model.
    pub fn new(rowid: i32) -> Self {
        RowidTest { rowid }
    }
}
