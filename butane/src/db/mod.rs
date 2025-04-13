//! Types, traits, and methods for interacting with a database.
//!
//! The different ways of referring to a database handle may present
//! some initial confusion.
//! * `ConnectionMethods` is a trait containing the methods available on a database connection or a transaction.
//!   Most methods on a [DataObject][crate::DataObject] or a [Query][crate::query::Query] require an
//!   implementation of `ConnectionMethods`.
//! * `BackendConnection` is a trait representing a direct connection to a database backend. It is a superset
//!   of `ConnectionMethods` and also includes the ability to create a transaction.
//! * `Transaction` is a struct representing a database transaction. It implements `ConnectionMethods`.
//! * `Connection` is a convenience struct containing a boxed `BackendConnection`. It cannot do anything other than
//!   what a `BackendConnection` can do, but allows using a single concrete type that is not tied to a particular
//!   database backend. It is returned by the `connect` method.

pub use butane_core::db::*;

#[cfg(feature = "r2d2")]
mod r2;

#[cfg(feature = "deadpool")]
mod deadpool;

/// Connection manager used with connection pooling systems such as r2d2 or deadpool.
/// With the `r2d2` feature enabled, it implements `r2d2::ManageConnection`.
/// With the `deadpool` feature enabled, it implements `deadpool::managed::Manager`.
#[cfg(any(feature = "deadpool", feature = "r2d2"))]
#[derive(Clone, Debug)]
pub struct ConnectionManager {
    spec: ConnectionSpec,
}
#[cfg(any(feature = "deadpool", feature = "r2d2"))]
impl ConnectionManager {
    /// Create a new ConnectionManager from a [ConnectionSpec].
    pub fn new(spec: ConnectionSpec) -> Self {
        ConnectionManager { spec }
    }
}
