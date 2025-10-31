use butane::db::{Connection, ConnectionAsync};
use butane::migrations::Migrations;
use butane_test_helper::*;
use butane_test_macros::butane_test;

use fixed_string::{User, Product, Order, Config, Session};

#[maybe_async_cfg::maybe(
    sync(),
    async(),
    idents(
        Connection(sync = "Connection", async = "ConnectionAsync"),
        DataObjectOps(sync = "DataObjectOpsSync", async = "DataObjectOpsAsync"),
    )
)]
async fn insert_data(connection: &Connection) {
    use butane::DataObjectOps;

    // Test that tables exist and work with ArrayString fields by performing CRUD operations
    // (the migrations should have been applied before this function is called)

    // Create a user with ArrayString fields
    let mut user = User::new("alice", "alice@example.com").unwrap();
    user = user.with_display_name("Alice Smith").unwrap();
    user = user.with_status("active").unwrap();
    user.save(connection).await.unwrap();

    // Verify user was saved correctly
    let saved_user = User::get(connection, user.id).await.unwrap();
    assert_eq!(saved_user.username.as_str(), "alice");
    assert_eq!(saved_user.email.as_str(), "alice@example.com");
    assert_eq!(saved_user.display_name.as_ref().unwrap().as_str(), "Alice Smith");
    assert_eq!(saved_user.status.as_str(), "active");

    // Create a product with ArrayString primary key
    let mut product = Product::new("WIDGET-001", "Super Widget", "electronics", 2999).unwrap();
    product.save(connection).await.unwrap();

    let saved_product = Product::get(connection, product.sku.clone()).await.unwrap();
    assert_eq!(saved_product.name.as_str(), "Super Widget");
    assert_eq!(saved_product.category.as_str(), "electronics");

    // Create an order referencing the user and product
    let mut order = Order::new("ORD-001", saved_user.clone(), saved_product.clone(), 2).unwrap();
    order.save(connection).await.unwrap();

    let saved_order = Order::get(connection, order.id).await.unwrap();
    assert_eq!(saved_order.order_number.as_str(), "ORD-001");
    assert_eq!(saved_order.quantity, 2);
    assert_eq!(saved_order.status.as_str(), "pending");

    // Create a config entry with ArrayString primary key
    let mut config = Config::new("max_connections", "100").unwrap();
    config = config.with_description("Maximum database connections").unwrap();
    config.save(connection).await.unwrap();

    let saved_config = Config::get(connection, config.key.clone()).await.unwrap();
    assert_eq!(saved_config.value.as_str(), "100");
    assert_eq!(saved_config.description.as_ref().unwrap().as_str(), "Maximum database connections");

    // Create a session with ArrayString primary key
    let user_id_value = saved_user.id.expect("User ID should be set after saving");
    
    let mut session = Session::new(
        "sess_1234567890abcdef",
        user_id_value,
        "192.168.1.1",
        "Mozilla/5.0 (Test Browser)"
    ).unwrap();
    session = session.with_device_fingerprint("fp_device123").unwrap();
    session.save(connection).await.unwrap();

    let saved_session = Session::get(connection, session.session_id.clone()).await.unwrap();
    assert_eq!(saved_session.user_id, user_id_value);
    assert_eq!(saved_session.ip_address.as_str(), "192.168.1.1");
    assert_eq!(saved_session.user_agent.as_str(), "Mozilla/5.0 (Test Browser)");
    assert_eq!(saved_session.status.as_str(), "active");
    assert_eq!(saved_session.device_fingerprint.as_ref().unwrap().as_str(), "fp_device123");
}

#[test_log::test(butane_test(async, nomigrate, pg))]
async fn migrate_and_unmigrate_async(mut connection: ConnectionAsync) {
    // Migrate forward.
    let base_dir = std::path::PathBuf::from(".butane");
    let migrations = butane_cli::get_migrations(&base_dir).unwrap();

    migrations.migrate_async(&mut connection).await.unwrap();

    insert_data_async(&connection).await;

    // Undo migrations.
    migrations.unmigrate_async(&mut connection).await.unwrap();
}

#[butane_test(sync, nomigrate)]
fn migrate_and_unmigrate_sync(mut connection: Connection) {
    // Migrate forward.
    let base_dir = std::path::PathBuf::from(".butane");
    let migrations = butane_cli::get_migrations(&base_dir).unwrap();

    migrations.migrate(&mut connection).unwrap();

    insert_data_sync(&connection);

    // Undo migrations.
    migrations.unmigrate(&mut connection).unwrap();
}