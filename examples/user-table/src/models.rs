//! Models for the user-table example.

use butane::model;
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
