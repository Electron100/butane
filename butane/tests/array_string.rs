//! Integration tests for ArrayString support in butane

use arrayvec::ArrayString;
use butane::db::{Connection, ConnectionAsync};
use butane::{model, AutoPk};
use butane_test_helper::*;
use butane_test_macros::butane_test;

#[model]
#[derive(Debug, Clone, PartialEq)]
struct User {
    id: AutoPk<i64>,
    /// Fixed-size username field (max 32 characters)
    username: ArrayString<32>,
    /// Fixed-size email field (max 255 characters)
    email: ArrayString<255>,
    /// Optional fixed-size display name (max 64 characters)
    display_name: Option<ArrayString<64>>,
}

impl User {
    fn new(username: &str, email: &str) -> Self {
        let mut user = User {
            id: AutoPk::uninitialized(),
            username: ArrayString::new(),
            email: ArrayString::new(),
            display_name: None,
        };
        user.username.push_str(username);
        user.email.push_str(email);
        user
    }
}

#[model]
#[derive(Debug, Clone, PartialEq)]
struct Product {
    /// Use ArrayString as primary key
    #[pk]
    sku: ArrayString<16>,
    name: ArrayString<128>,
    description: Option<ArrayString<512>>,
}

impl Product {
    fn new(sku: &str, name: &str) -> Self {
        let mut product = Product {
            sku: ArrayString::new(),
            name: ArrayString::new(),
            description: None,
        };
        product.sku.push_str(sku);
        product.name.push_str(name);
        product
    }

    fn with_description(mut self, description: &str) -> Self {
        let mut desc = ArrayString::new();
        desc.push_str(description);
        self.description = Some(desc);
        self
    }
}

#[butane_test]
async fn test_array_string_crud_operations(conn: ConnectionAsync) {
    // Create a user with ArrayString fields
    let mut user = User::new("alice", "alice@example.com");
    user.save(&conn).await.expect("Failed to save user");

    // Verify the user was saved with correct values
    let saved_user = User::get(&conn, user.id).await.expect("Failed to get user");
    assert_eq!(saved_user.username.as_str(), "alice");
    assert_eq!(saved_user.email.as_str(), "alice@example.com");
    assert_eq!(saved_user.display_name, None);

    // Update the user with a display name
    let mut updated_user = saved_user;
    updated_user.display_name = Some({
        let mut name = ArrayString::new();
        name.push_str("Alice Smith");
        name
    });
    updated_user
        .save(&conn)
        .await
        .expect("Failed to update user");

    // Verify the update
    let final_user = User::get(&conn, updated_user.id)
        .await
        .expect("Failed to get updated user");
    assert_eq!(
        final_user.display_name.as_ref().unwrap().as_str(),
        "Alice Smith"
    );
}

#[butane_test]
async fn test_array_string_as_primary_key(conn: ConnectionAsync) {
    // Create products with ArrayString SKU as primary key
    let mut product1 = Product::new("ABC123", "Widget A");
    let mut product2 = Product::new("XYZ789", "Widget B")
        .with_description("A high-quality widget for all your needs");

    product1.save(&conn).await.expect("Failed to save product1");
    product2.save(&conn).await.expect("Failed to save product2");

    // Retrieve by primary key
    let retrieved1 = Product::get(&conn, {
        let mut sku: ArrayString<16> = ArrayString::new();
        sku.push_str("ABC123");
        sku
    })
    .await
    .expect("Failed to get product1");

    assert_eq!(retrieved1.name.as_str(), "Widget A");
    assert_eq!(retrieved1.description, None);

    let retrieved2 = Product::get(&conn, {
        let mut sku: ArrayString<16> = ArrayString::new();
        sku.push_str("XYZ789");
        sku
    })
    .await
    .expect("Failed to get product2");

    assert_eq!(retrieved2.name.as_str(), "Widget B");
    assert_eq!(
        retrieved2.description.as_ref().unwrap().as_str(),
        "A high-quality widget for all your needs"
    );
}

#[butane_test]
async fn test_array_string_length_limits(conn: ConnectionAsync) {
    // Test that we can store strings up to the ArrayString capacity
    let max_username = "a".repeat(32); // exactly 32 chars
    let max_email = "a".repeat(243) + "@example.com"; // 243 + 12 = 255 chars total

    let mut user = User {
        id: AutoPk::uninitialized(),
        username: ArrayString::from(&max_username).expect("Username should fit"),
        email: ArrayString::from(&max_email).expect("Email should fit"),
        display_name: Some(ArrayString::from(&"a".repeat(64)).expect("Display name should fit")),
    };

    user.save(&conn)
        .await
        .expect("Failed to save user with max length fields");

    let saved_user = User::get(&conn, user.id)
        .await
        .expect("Failed to retrieve user");
    assert_eq!(saved_user.username.as_str(), max_username);
    assert_eq!(saved_user.email.as_str(), max_email);
    assert_eq!(
        saved_user.display_name.as_ref().unwrap().as_str(),
        "a".repeat(64)
    );
}

#[butane_test]
async fn test_array_string_empty_values(conn: ConnectionAsync) {
    // Test empty ArrayString values
    let mut user = User {
        id: AutoPk::uninitialized(),
        username: ArrayString::new(), // empty
        email: ArrayString::from("empty@example.com").unwrap(),
        display_name: Some(ArrayString::new()), // empty but not null
    };

    user.save(&conn)
        .await
        .expect("Failed to save user with empty username");

    let saved_user = User::get(&conn, user.id).await.expect("Failed to get user");
    assert_eq!(saved_user.username.as_str(), "");
    assert_eq!(saved_user.email.as_str(), "empty@example.com");
    assert_eq!(saved_user.display_name.as_ref().unwrap().as_str(), "");
}

#[butane_test]
async fn test_array_string_special_characters(conn: ConnectionAsync) {
    // Test ArrayString with special characters and Unicode
    let special_username = "user_123!@#";
    let unicode_email = "tëst@émáil.com";
    let emoji_display = "User 👤 Name";

    let mut user = User {
        id: AutoPk::uninitialized(),
        username: ArrayString::from(special_username).expect("Should fit"),
        email: ArrayString::from(unicode_email).expect("Should fit"),
        display_name: Some(ArrayString::from(emoji_display).expect("Should fit")),
    };

    user.save(&conn)
        .await
        .expect("Failed to save user with special characters");

    let saved_user = User::get(&conn, user.id).await.expect("Failed to get user");
    assert_eq!(saved_user.username.as_str(), special_username);
    assert_eq!(saved_user.email.as_str(), unicode_email);
    assert_eq!(
        saved_user.display_name.as_ref().unwrap().as_str(),
        emoji_display
    );
}
