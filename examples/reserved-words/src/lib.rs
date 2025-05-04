//! Common helpers for the newtype example.

#![deny(missing_docs)]

pub mod butane_migrations;
pub mod models;

use butane::db::{ConnectionAsync, ConnectionSpec};
use butane::migrations::Migrations;
pub use models::User;

/// Load a [Connection].
pub async fn establish_connection() -> ConnectionAsync {
    let mut connection =
        butane::db::connect_async(&ConnectionSpec::load(".butane/connection.json").unwrap())
            .await
            .unwrap();
    let migrations = butane_migrations::get_migrations().unwrap();
    migrations.migrate_async(&mut connection).await.unwrap();
    connection
}
