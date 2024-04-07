//! Models for the newtype example.

use butane::{model, FieldType, ForeignKey, PrimaryKeyType};
use garde::Validate;
use serde::{Deserialize, Serialize};

/// Blog identifier.
#[derive(Clone, Debug, Default, Deserialize, Eq, FieldType, PartialEq, Serialize)]
pub struct BlogId(pub uuid::Uuid);
impl PrimaryKeyType for BlogId {}

/// Post identifier.
#[derive(Clone, Debug, Default, Deserialize, Eq, FieldType, PartialEq, Serialize)]
pub struct PostId(pub uuid::Uuid);
impl PrimaryKeyType for PostId {}

/// Blog name.
#[derive(Clone, Debug, Default, Deserialize, Eq, FieldType, PartialEq, Serialize, Validate)]
pub struct BlogName(#[garde(ascii)] String);

/// Blog post unique tags.
#[derive(Clone, Debug, Default, Deserialize, Eq, FieldType, PartialEq, Serialize)]
pub struct Tags(pub std::collections::HashSet<String>);

/// Blog metadata.
#[model]
#[derive(Debug, Default, Validate)]
pub struct Blog {
    /// Id of the blog.
    #[garde(skip)]
    pub id: BlogId,
    /// Name of the blog.
    #[garde(dive)]
    pub name: BlogName,
}
impl Blog {
    /// Create a new Blog.
    pub fn new(name: impl Into<String>) -> Result<Self, garde::Report> {
        let blog = Blog {
            id: BlogId(uuid::Uuid::new_v4()),
            name: BlogName(name.into()),
        };
        blog.validate(&())?;
        Ok(blog)
    }
}
/// Post details, including a [ForeignKey] to [Blog]
/// and storing tags in [Tags] JSON field.
#[model]
pub struct Post {
    /// Id of the blog post.
    pub id: PostId,
    /// Title of the blog post.
    pub title: String,
    /// Body of the blog post.
    pub body: String,
    /// Whether the blog post has been published.
    pub published: bool,
    /// Tags for the blog post.
    pub tags: Tags,
    /// The [Blog] this post is attached to.
    pub blog: ForeignKey<Blog>,
    /// Byline of the post.
    pub byline: Option<String>,
    /// How many likes this post has.
    pub likes: i32,
}
impl Post {
    /// Create a new Post.
    pub fn new(blog: &Blog, title: String, body: String) -> Self {
        Post {
            id: PostId(uuid::Uuid::new_v4()),
            title,
            body,
            published: false,
            tags: Tags::default(),
            blog: blog.into(),
            byline: None,
            likes: 0,
        }
    }
}
