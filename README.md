[![Latest Version](https://img.shields.io/crates/v/butane.svg)](https://crates.io/crates/butane)
[![docs](https://docs.rs/butane/badge.svg)](https://docs.rs/butane)
[![Build Status](https://img.shields.io/github/actions/workflow/status/Electron100/butane/ci.yml?branch=master)](https://github.com/Electron100/butane/actions?query=branch%3Amaster)

# Butane

**An experimental ORM for Rust with a focus on simplicity and on writing Rust, not SQL**

Butane takes an object-oriented approach to database operations. It
may be thought of as much as an object-persistence system as an ORM --
the fact that it is backed by a SQL database is mostly an
implementation detail to the API consumer.

## Features

* Relational queries using Rust-like syntax (via `proc-macro`s)
* Automatic migrations without writing SQL (although the generated SQL
  may be hand-tuned if necessary)
* Ability to embed migrations in Rust code (so that a library may easily bundle its migrations)
* SQLite and PostgreSQL backends
* Write entirely or nearly entirely the same code regardless of database backend
  
## Getting Started

_Models_, declared with struct attributes define the database
schema. For example the Post model for a blog might look like this:

``` rust
#[model]
#[derive(Default)]
struct Post {
    id: AutoPk<i32>,
    title: String,
    body: String,
    published: bool,
    likes: i32,
    tags: Many<Tag>,
    blog: ForeignKey<Blog>,
    byline: Option<String>,
}
```

An _object_ is an instance of a _model_. An object is created like a
normal struct instance, but must be saved in order to be persisted.

``` rust
let mut post = Post::new(blog, title, body);
post.save(conn)?;
```

Changes to the instance are only applied to the database when saved:

``` rust
post.published = true;
post.save(conn)?;
```

Queries are performed ergonomically with the `query!` macro.

``` rust
let posts = query!(Post, published == true).limit(5).load(&conn)?;
```

For a detailed tutorial, see the [Getting Started Guide](https://electron100.github.io/butane/getting-started).

## Cargo Features

Butane exposes several features to Cargo. By default, no backends are
enabled: you will want to enable `sqlite` and/or `pg`:

* `default`: Turns on `datetime`, `json` and `uuid`.
* `debug`: Used in developing Butane, not expected to be enabled by consumers.
* `datetime`: Support for timestamps (using [`chrono`](https://crates.io/crates/chrono) crate).
* `fake`: Support for the [`fake`](https://crates.io/crates/fake) crate's generation of fake data.
* `json`: Support for storing structs as JSON, including using postgres' `JSONB` field type.
* `log`: Log certain warnings to the [`log`](https://crates.io/crates/log) crate facade (target "butane").
* `pg`: Support for PostgreSQL using [`postgres`](https://crates.io/crates/postgres) crate.
* `r2d2`: Connection pooling using [`r2d2`](https://crates.io/crates/r2d2) support
  (See `butane::db::ConnectionManager`).
* `sqlite`: Support for SQLite using [`rusqlite`](https://crates.io/crates/rusqlite) crate.
* `sqlite-bundled`: Bundles sqlite instead of using the system version.
* `tls`: Support for TLS when using PostgreSQL, using
  [`postgres-native-tls`](https://crates.io/crates/postgres-native-tls) crate.
* `uuid`: Support for UUIDs (using the [`uuid`](https://crates.io/crates/uuid) crate).

## Limitations

* Butane, and its migration system especially, expects to own the
  database. It can be used with an existing database accessed also by
  other consumers, but it is not a design goal and there is no
  facility to infer butane models from an existing database schema.
* API ergonomics are prioritized above performance. This does not mean
  Butane is slow, but that when given a choice between a simple,
  straightforward API and eking out the smallest possible overhead,
  the API will win.

## Roadmap

Butane is young. The following features are currently missing, but planned

* Foreign key constraint cascade setting
* Incremental object save
* Back-references for `ForeignKey` and `Many`.
* Field/column rename support in migrations
* Prepared/reusable queries
* Benchmarking and performance tuning
* Support for other databases such as MySQL or SQL Server are not
  explicitly planned, but contributions are welcome.

## Comparison to Diesel

Butane is inspired by Diesel and by Django's ORM. If you're looking
for a mature, performant, and flexible ORM, go use Diesel. Butane
doesn't aim to be better than Diesel, but makes some _different_ decisions, including:

1. It is more object-oriented, at the cost of flexibility.
2. Automatic migrations are prioritized.
3. Rust code is the source of truth. The schema is understood from the
   definition of Models in Rust code, rather than inferred from the
   database.
4. Queries are constructed using a DSL inside a `proc-macro` invocation
   rather than by importing DSL methods/names to use into the current
   scope. For Diesel, you might write

   ```rust
   use diesel_demo::schema::posts::dsl::*;
   let posts = posts.filter(published.eq(true))
        .limit(5)
        .load::<Post>(&conn)?
   ```

   whereas for Butane, you would instead write

   ```rust
   let posts = query!(Post, published == true).limit(5).load(&conn)?;
   ```

   Which form is preferable is primarily an aesthetic
   judgement.
5. Differences between database backends are largely hidden.
6. Diesel is overall significantly more mature and full-featured.

For a detailed tutorial, see [the getting started
guide](https://electron100.github.io/butane/getting-started).

## License

Butane is licensed under either of the [MIT license](LICENSE-MIT) or
the [Apache License, Version 2.0](LICENSE-APACHE) at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in Butane by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms
or conditions.
