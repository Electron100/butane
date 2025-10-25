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

**Problem**: Turso (libSQL) does not currently support the `IN (...subquery)` syntax in WHERE clauses.

This causes failures when loading many-to-many relationships using the `.load()` method.

**Example Failing Code**:

```rust
// This will fail on Turso backend
let post_from_db = find_async!(Post, id == { post.id }, &conn).unwrap();
let tags = post_from_db.tags.load(&conn).await.unwrap(); // ERROR here
```

**Error Message**:

```text
SQL execution failure: `Parse error: IN (...subquery) in WHERE clause is not supported`
```

**Root Cause**: When loading many-to-many relationships, Butane generates SQL like:

```sql
SELECT * FROM tag WHERE id IN (SELECT has FROM post_tag WHERE owner = ?)
```

This SQL pattern is not supported by Turso's current libSQL version.

**Possible Workarounds**:

Until this is fixed in Butane's Turso backend, consider these alternatives:

1. **Manual JOIN Query**: Query the relationship manually using a JOIN:

   ```rust
   // Instead of post.tags.load(&conn).await
   // Manually query with a join (requires custom SQL or query builder enhancement)
   ```

2. **Load IDs and Query Separately**:

   ```rust
   // Query the junction table first
   // Then query the related objects with IN (values) instead of IN (subquery)
   ```

3. **Use EXISTS Instead**: A future fix could transform the query to:

   ```sql
   SELECT * FROM tag WHERE EXISTS (
     SELECT 1 FROM post_tag
     WHERE post_tag.has = tag.id AND post_tag.owner = ?
   )
   ```

**Planned Solution**: The Butane maintainers are aware of this limitation.

The planned fix involves:

- Adding backend-specific SQL generation for Turso
- Transforming `BoolExpr::Subquery` into a JOIN or EXISTS-based query for Turso
- This will happen automatically without requiring changes to user code

**Affected Code Locations** (for contributors):

- `butane_core/src/many.rs` line 119: Where `BoolExpr::Subquery` is generated
- `butane_core/src/db/helper.rs` lines 93-109: Where the SQL is generated
- `butane_core/src/db/turso.rs` line 156+: Where Turso-specific query logic could be added

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
