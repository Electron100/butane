# Newtype support

This guide builds on the `getting_started` walkthrough, using the
[newtype pattern](https://rust-unofficial.github.io/patterns/patterns/behavioural/newtype.html)
with Butane to utilise types that are not natively supported by Butane.

It again builds the database portions of a blog, so that the base models
are not new, and the focus can be on the additional types used.

Let's begin by creating a new rust project

``` shell
cargo new --lib newtype && cd newtype
```

In `Cargo.toml`, add a dependency on Butane, and a few other types we'll use:

``` toml
[dependencies]
butane = { version = "0.6", features=["pg", "sqlite"] }
uuid = { version = "1.8", features = ["serde", "v4"] }
```

This guide will use SQLite initially, and use "pg" for
PostgreSQL support at the end.

Again, we initialise the butane metadata with

``` shell
cargo install butane_cli
butane init sqlite example.db
```

Copy the `lib.rs` from the `getting_started` example into `src/`.

## Wrapping supported types

### Uuids

Let's enhance the `Blog` and `Post` models to have dedicated primary key types, and are a `uuid`.

Create `src/models.rs` with a `BlogId` and `PostId` struct wrapping a `uuid`.

``` rust
use butane::{FieldType, PrimaryKeyType};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, FieldType, PartialEq, Eq)]
pub struct BlogId(pub uuid::Uuid);
impl PrimaryKeyType for BlogId {}

#[derive(Clone, Debug, Default, Deserialize, Serialize, FieldType, PartialEq, Eq)]
pub struct PostId(pub uuid::Uuid);
impl PrimaryKeyType for PostId {}
```

For each, `FieldType` is derived, and they implement the marker trait `PrimaryKeyType`
to allow their use as a primary key.

As Butane natively supports `uuid`, these newtypes will be stored in the butane metadata as
"Blob" type, which is stored in the database using an appropriate column type based on the
database's supported columns.  For `uuid`, this is a `BLOB` on SQLite, and `BYTEA` on PostgreSQL.

If we compile this project now, a `.butane/migrations/current/types.json` will be generated
with the following contents:

``` json
{"CT:BlogId":{"KnownId":{"Ty":"Blob"}},"CT:PostId":{"KnownId":{"Ty":"Blob"}}}
```

Now we can add `Blog` and `Post`, which can use these types for their primary key.

``` rust
#[model]
#[derive(Debug, Default)]
pub struct Blog {
    pub id: BlogId,
    // ...
}

#[model]
pub struct Post {
    pub id: PostId,
    // ...
}
```

Now it is impossible to accidentally use a `BlogId` in conjunction with `Post.id`.

### Strings

We know that Unicode contains lots of [Homoglyph](https://en.wikipedia.org/wiki/Homoglyph),
so a simple way to avoid two blogs having indistinguishable names is to require the name is ASCII.

We can use [`garde`](https://crates.io/crates/garde) to add validation.

``` rust
#[derive(Clone, Debug, Default, Deserialize, Dummy, Eq, FieldType, PartialEq, Serialize, Validate)]
pub struct BlogName(#[garde(ascii)] String);
```

If we compile this project now, `.butane/migrations/current/types.json` will contain a new entry
`CT:BlogName`, which is of type "Text":

``` json
{"CT:BlogId":{"KnownId":{"Ty":"Blob"}},"CT:BlogName":{"KnownId":{"Ty":"Text"}},"CT:PostId":{"KnownId":{"Ty":"Blob"}}}
```

### Unsupported types

The previous two "newtypes" wrapped types supported by Butane.
When Butane does not support the inner type, `#[derive(FieldType)]`
will fallback to storing the type in JSON.

We can use this to implement the blog post tags without a separate table, using `HashSet`.

``` rust
#[derive(Clone, Debug, Default, Deserialize, Dummy, Eq, FieldType, PartialEq, Serialize)]
pub struct Tags(pub std::collections::HashSet<String>);
```

If we compile this project now, `.butane/migrations/current/types.json` will contain a new entry
`CT:Tags`, which is of type "Json":

``` json
{"CT:BlogId":{"KnownId":{"Ty":"Blob"}},"CT:BlogName":{"KnownId":{"Ty":"Text"}},"CT:PostId":{"KnownId":{"Ty":"Blob"}}}
```

In SQLite, this will be stored in a "TEXT" column, while on PostgreSQL it
will use the "JSONB" column type.

This can now be used in the Post struct:

``` rust
#[model]
pub struct Post {
    // ..
    pub tags: Tags,
    // ..
}
```
