# Turso Backend for Butane

This document describes the Turso backend integration for Butane ORM.

## Overview

[Turso](https://github.com/tursodatabase/turso) is an in-process SQL database written in Rust,
compatible with SQLite.

The Turso backend for Butane leverages this SQLite compatibility while providing async-first
database operations.

## Features

- **Async-first**: Turso is built for async operations from the ground up
- **SQLite Compatible**: Uses the same SQL dialect and features as SQLite
- **Memory and File-based**: Supports both in-memory (`:memory:`) and file-based databases
- **Migration Support**: Full support for Butane's migration system
- **Transaction Support**: ACID-compliant transaction support

## Installation

Add Turso support to your `Cargo.toml`:

```toml
[dependencies]
butane = { version = "0.8", features = ["async", "turso"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Usage

### Basic Connection

```rust
use butane::db::turso::TursoBackend;
use butane::prelude_async::*;

#[tokio::main]
async fn main() -> Result<(), butane::Error> {
    // In-memory database
    let backend = TursoBackend::new();
    let mut conn = backend.connect_async(":memory:").await?;

    // Or file-based database
    let mut conn = backend.connect_async("my_database.db").await?;

    Ok(())
}
```

### Using with Butane Models

```rust
use butane::prelude_async::*;
use butane::{model, DataResult};

#[model]
#[derive(Debug)]
struct Post {
    #[auto]
    id: i64,
    title: String,
    body: String,
    published: bool,
}

#[tokio::main]
async fn main() -> Result<(), butane::Error> {
    let backend = butane::db::turso::TursoBackend::new();
    let mut conn = backend.connect_async(":memory:").await?;

    // Run migrations
    butane::migrations::from_root()
        .await?
        .migrate_async(&mut conn)
        .await?;

    // Create a new post
    let mut post = Post {
        id: 0,
        title: "Hello Turso!".to_string(),
        body: "This is a post using the Turso backend.".to_string(),
        published: true,
    };
    post.save_async(&mut conn).await?;

    // Query posts
    let posts = Post::query()
        .filter(Post::published().eq(true))
        .load_async(&conn)
        .await?;

    for post in posts {
        println!("{}: {}", post.title, post.body);
    }

    Ok(())
}
```

### Transactions

```rust
use butane::prelude_async::*;

async fn update_with_transaction(
    conn: &mut butane::db::ConnectionAsync
) -> Result<(), butane::Error> {
    let mut tx = conn.transaction().await?;

    // Perform operations within transaction
    // ...

    // Commit or rollback
    tx.commit().await?;
    // Or: tx.rollback().await?;

    Ok(())
}
```

## Testing

The Turso backend includes full test support through `butane_test_helper`:

```rust
use butane_test_helper::*;

async fn my_test(conn: ConnectionAsync) {
    // Your test code here
}

#[tokio::test]
async fn test_with_turso() {
    maketest_turso!(my_test, true);
}
```

## Differences from SQLite Backend

1. **Async-Only**: Unlike the SQLite backend which supports both sync and async, Turso only supports async operations
2. **Native Rust**: Turso is written in Rust, while the SQLite backend uses the C-based rusqlite library
3. **Modern Architecture**: Built with async I/O support from the ground up

## Migration from SQLite

Since Turso is SQLite-compatible, migrating from SQLite is straightforward:

1. Change your backend from `SQLiteBackend` to `TursoBackend`
2. Use async methods (`connect_async`, `save_async`, etc.) instead of sync methods
3. Your existing migrations and models work without modification

## Limitations

- **Sync Operations**: Turso does not support synchronous operations. All database operations must be async.
- **Custom Types**: Like SQLite, custom SQL types are not supported (use JSON serialization instead)
- **Extensions**: SQLite extensions are not available in Turso
- **Subquery in WHERE clause**: Turso/libSQL does not support `IN (...subquery)` syntax in WHERE clauses.

## Known Issues

### Relationship Queries

**Status**: ✅ **FIXED** - Many-to-many relationship queries now work correctly with the Turso backend.

**Solution**: The Turso backend now automatically transforms subquery expressions into equivalent queries
that Turso supports. When loading many-to-many relationships, the backend executes the subquery first
to get a list of IDs, then uses those IDs in an `IN (value1, value2, ...)` clause instead of
`IN (...subquery)`.

**Technical Details**: Turso/libSQL does not support subqueries in WHERE clauses (`IN (...subquery)`
or `EXISTS (...)`). The fix intercepts `BoolExpr::Subquery` and `BoolExpr::SubqueryJoin` expressions
before SQL generation and transforms them by:

1. Executing the subquery separately to retrieve matching values
2. Converting the result into a `BoolExpr::In` expression with concrete values
3. Generating standard SQL with `IN (val1, val2, ...)` which Turso supports

This transformation is transparent to user code - all many-to-many operations work as expected:

```rust
// This now works correctly on Turso backend
let post_from_db = find_async!(Post, id == { post.id }, &conn).unwrap();
let tags = post_from_db.tags.load(&conn).await.unwrap(); // ✅ Works!
```

**Implementation**: See `butane_core/src/db/turso.rs` - the `transform_subqueries` method handles the conversion.

### Table Rename Migration

**Problem**: The newtype example fails when attempting to rename tables during migration with the error:

```text
table being renamed should be in schema
```

**Root Cause**: This is a limitation in Turso/libSQL's internal schema tracking. When a table is created
and then renamed within the same transaction, libSQL's schema registry doesn't properly track the
intermediate state. The issue occurs in migrations that use the create-copy-drop-rename pattern to
alter table schemas (such as changing column definitions).

**Technical Details**: The `change_column` operation in Butane generates SQL like:

```sql
CREATE TABLE Post__butane_tmp (...);  -- Create temp table
INSERT INTO Post__butane_tmp SELECT ... FROM Post;  -- Copy data
DROP TABLE Post;  -- Drop original
ALTER TABLE Post__butane_tmp RENAME TO Post;  -- Rename temp to original
```

The final `ALTER TABLE ... RENAME TO` fails because libSQL doesn't recognize `Post__butane_tmp`
as being in the schema, even though it was just created in a previous statement within the same
transaction.

**Affected Examples**:

- `examples/newtype/` - Unmigrate operations that require column changes

**Workaround**: Tests that perform unmigrate operations with column changes now skip execution for
the Turso backend. The migrations themselves work correctly in the forward direction; only the
unmigrate (downgrade) operations are affected when they involve schema changes that require the
create-copy-drop-rename pattern.

**Status**: This is a known limitation in libSQL 0.2.x. Potential solutions include:

- Wait for libSQL updates that improve schema tracking within transactions
- Modify Butane's migration system to execute schema-changing statements outside of transactions for Turso
- Use a different approach for column changes that doesn't require table renames

For most use cases, this limitation has minimal impact since forward migrations work correctly and
downgrading migrations is rarely needed in production environments.

## Performance

Turso is designed for high performance with:

- Native async I/O support
- Modern Rust implementation
- Efficient memory management
- Vector search capabilities (in development)

## Contributing

Contributions to improve the Turso backend are welcome! Please see the main Butane repository for
contribution guidelines.

## Resources

- [Turso Database](https://github.com/tursodatabase/turso)
- [Butane ORM](https://github.com/Electron100/butane)
- [Turso Documentation](https://github.com/tursodatabase/turso/blob/main/docs/manual.md)

## License

The Turso backend for Butane is licensed under the same terms as Butane itself (MIT OR Apache-2.0).
