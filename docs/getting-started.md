# Getting Started

This guide gives a brief, high-level overview of the basic CRUD
operations and features of Butane. The complete code can be found at
[examples/getting_started](https://github.com/Electron100/butane/tree/master/examples/getting_started). We
deliberately follow the same goal as [Diesel's getting-started
guide](https://diesel.rs/guides/getting-started/): building the
database portions of a blog.

Let's begin by creating a new rust project

``` shell
cargo new --lib getting_started && cd getting_started
```

In `Cargo.toml`, add a dependency on Butane:

``` toml
[dependencies]
butane = { version = "0.6", features=["pg", "sqlite"] }
```

This guide will focus on using SQLite initially, and use "pg" for
PostgreSQL support at the end.

A word on error-handling: for simplicity, this example unwraps errors
to panic on failure. In a real program, you would of course handle
your errors.

## Initialization

Butane provides a CLI to help with database connection and
migration. It's optional -- it uses only public Butane APIs -- but it
helps with common tasks. Let's install it and initialize our
database. It's intended to be run from the same directory as the
Cargo package (i.e. the one containing `Cargo.toml`).

``` shell
cargo install butane_cli
butane init sqlite example.db
```

This will have created an `example.db` SQLite file in the current
directory as well as a `.butane` subdirectory. Inside that
subdirectory, we see a `connection.json` file containing our
connection parameters.

## Connection

At this point, we can add a method (in our
`lib.rs`) to establish a connection in code.

``` rust
use butane::db::{Connection, ConnectionSpec};

pub fn establish_connection() -> Connection {
    butane::db::connect(&ConnectionSpec::load(".butane/connection.json").unwrap()).unwrap()
}
```

## Models

We can connect to our database, but we can't really do anything
yet. Let's define some _models_ for our blog objects (in `src/models.rs`). We'll start with
the Blog itself.

``` rust
use butane::prelude::*;
use butane::{model, ForeignKey, Many, ObjectState};

#[model]
#[derive(Debug, Default)]
pub struct Blog {
    pub id: AutoPk<i64>,
    pub name: String,
}
impl Blog {
    pub fn new(name: impl Into<String>) -> Self {
        Blog {
            id: AutoPk::uninitialized(),
            name: name.into()
        }
    }
}
```

The `#[model]` attribute does the heavy lifting here:

1. It generates automatic `impl`s of [`butane::DataResult`] and
   [`butane::DataObject`].
2. It adds an additional field `state: butane::ObjectState` used to
   store internal Butane state information. In general, we can ignore
   this field, but it must be initialized when the struct is
   constructed and there may not be another field named `state`,
   although it is acceptable to manually include the `state:
   ObjectState` field in the struct definition to make its presence
   more obvious (and rust-analyzer happier).
3. It tells Butane that instances of this struct should be represented in the
   database, recording migration info (more on this later).

The `id` field is special -- it's the primary key. All models must
have a primary key. If we didn't want to name ours `id`, we could have
added a `#[pk]` attribute to denote the primary key field. The
`AutoPk` wrapping type says that the field should be populated
automatically from an incrementing value. It is only allowed on
integer types and will cause the underlying column to be
`AUTOINCREMENT` for SQLite or `SERIAL`/`BIGSERIAL` for
PostgreSQL. When the object is created in the database via its
[`save`] method, the `AutoPk` field will be updated to its initialized
value.

Now let's add a model to represent a blog post, and in the process take a look at a few more features.

``` rust
#[model]
pub struct Post {
    pub id: AutoPk<i32>,
    pub title: String,
    pub body: String,
    pub published: bool,
    pub blog: ForeignKey<Blog>,
    pub tags: Many<Tag>,
    pub byline: Option<String>,
    // listed for clarity, generated automatically if omitted
    state: butane::ObjectState,
}
impl Post {
    pub fn new(blog: &Blog, title: String, body: String) -> Self {
        Post {
            id: -1,
            title,
            body,
            published: false,
            blog: blog.into(),
            tags: Many::default(),
            byline: None,
            state: ObjectState::default(),
        }
    }
}
```

Each post is associated with a single blog, represented by the
`ForeignKey<Blog>`. Posts and tags, however, have a many-to-many
relationship, represented here by `Many<Tag>`.

The Tag model itself is trivial

``` rust
#[model]
#[derive(Debug, Default)]
pub struct Tag {
    #[pk]
    pub tag: String,
}
impl Tag {
    pub fn new(tag: impl Into<String>) -> Self {
        Tag {
            tag: tag.into(),
            ..Default::default()
        }
    }
}
```

Then we can use them in our `lib.rs`:

```rust
pub mod models;

pub use models::{Blog, Post};
```

Let's build our package now. If we look in the `.butane` directory,
it has new items! There's a `migrations/current` subdirectory
recording information about our models. These files are necessary for
migrations to work, but their format is not part of Butane's public
API.

## Initial Migration

Butane has recorded our current state, but no tables have been created
in the database yet! We need to create our first migration. It doesn't
matter what we name it, so let's call it "init".

``` shell
butane makemigration init
```

The migration is created using our supplied name and the current date. If we now run

``` shell
butane list
```

It prints our migration and tell us that it's "(not applied)". So let's go ahead and apply it!

``` shell
butane migrate
```

Now that the database matches our models, let's write some more code.

## Create

To create an object in the database, we just instantiate a struct as
normal, then save it. Let's write `create_blog` and `create_post`
methods (in `lib.rs`):

``` rust
use butane::db::{Connection, ConnectionSpec};
use butane::prelude::*;

pub fn create_blog(conn: &Connection, name: impl Into<String>) -> Blog {
    let mut blog = Blog::new(name);
    blog.save(conn).unwrap();
    blog
}

pub fn create_post(conn: &Connection, blog: &Blog, title: String, body: String) -> Post {
    let mut new_post = Post::new(blog, title, body);
    new_post.save(conn).unwrap();
    new_post
}
```

The `butane::prelude::*` import brings some common Butane traits into
scope. If you'd prefer to avoid star-imports, you can import the
necessary traits explicitly (in this case `use butane::{DataObject,
DataResult};`)

We don't need to create a new blog every time, if we have an existing
one we want to reuse it (for simplicity we'll only add one blog in
this example), so let's add a method to find that existing blog.

``` rust
pub fn existing_blog(conn: &Connection) -> Option<Blog> {
    Blog::query().load_first(conn).unwrap()
}
```

At this point we have everything we need to create a short program to
write a post. Let's add a `write_post` binary to `Cargo.toml`:

``` toml
[[bin]]
name = "write_post"
doc = false
```

And write its code (in `src/bin/write_post.rs`).

``` rust
use std::io::{stdin, Read};

use getting_started::*;

fn main() {
    let conn = establish_connection();

    let blog = match existing_blog(&conn) {
        Some(blog) => blog,
        None => {
            println!("Enter blog name");
            let name = readline();
            create_blog(&conn, name)
        }
    };

    println!("Enter post title");
    let title = readline();
    println!("\nEnter text for {} ({} when finished)\n", title, EOF);
    let mut body = String::new();
    stdin().read_to_string(&mut body).unwrap();

    let post = create_post(&conn, &blog, title, body);
    println!(
        "\nSaved unpublished post {} with id {}",
        post.title, post.id
    );
}

fn readline() -> String {
    let mut s = String::new();
    stdin().read_line(&mut s).unwrap();
    s.pop(); // Drop the newline
    s
}

#[cfg(not(windows))]
const EOF: &str = "CTRL+D";

#[cfg(windows)]
const EOF: &str = "CTRL+Z";

```

Let's run this (`cargo run --bin write_post`) and author our first post.

## Read

Ok, that's great, we put some data in the database, but at some point
we're going to want to get it back to display it. The most ergonomic and
type-safe way to construct Butane queries is to use the `query!` macro.
To find all published posts, we'd write:

``` rust
query!(Post, published == true)
```

The heavy lifting is actually done by the `filter!` macro. The above is just shorthand for.

``` rust
Post::query().filter(filter!(Post, published == true))
```

`filter!` creates a `butane::query::BoolExpr`, but does so in a more
ergonomic and type-safe manner. If we had a typo and wrote
`query!(Post, publish == true)` ("publish" instead of "published") we'd get a compiler error.

Let's add another binary to `Cargo.toml`, this one called `show_posts`, and write its code (in `src/bin/show_posts.rs`).

``` rust
use butane::query;

use getting_started::models::Post;
use getting_started::*;

fn main() {
    let conn = establish_connection();
    let results = query!(Post, published == true)
        .limit(5)
        .load(&conn)
        .expect("Error loading posts");
    println!("Displaying {} posts", results.len());
    for post in results {
        println!("{}", post.title);
        println!("----------\n");
        println!("{}", post.body);
 
 }
}
```

If we run it (`cargo run --bin show_posts`) we don't see any posts
though. That's because it only prints published posts, and we haven't
published our post yet.

## Update

Let's create yet another program, `publish_post`. It needs to be given
the id of a post to publish. To publish the post, we find it by id,
mark it as published, and save it again.

Add `publish_post` binary to `Cargo.toml`, and write its code (in `src/bin/publish_post.rs`).

``` rust
use std::env::args;

use butane::prelude::*;

use getting_started::models::Post;
use getting_started::*;

fn main() {
    let id = args()
        .nth(1)
        .expect("publish_post requires a post id")
        .parse::<i32>()
        .expect("Invalid ID");
    let conn = establish_connection();

    let mut post = Post::get(&conn, id).expect(&format!("Unable to find post {}", id));
    // Just a normal Rust assignment, no fancy set methods
    post.published = true;
    post.save(&conn).unwrap();
    println!("Published post {}", post.title);
}
```

Let's publish our first post: `cargo run --bin publish_post 1`. Now
when we run `show_posts` again, it should display our newly published
post!

## Delete

We've gotten most of the way through CRUD. For completeness, let's see
how to delete a post. This can be done with either the `delete` method
on `DataObject` (to delete an object we've already loaded) or (more
commonly) with the `delete` method on `Query` to delete directly.
Here's our `delete_post` program (in `src/bin/delete_post.rs`):

``` rust
use std::env::args;

use butane::query;

use getting_started::models::Post;
use getting_started::*;

fn main() {
    let target = args().nth(1).expect("Expected a target to match against");
    let pattern = format!("%{}%", target);

    let conn = establish_connection();
    let cnt = query!(Post, title.like({ pattern }))
        .delete(&conn)
        .expect("error deleting posts");
    println!("Deleted {} posts", cnt);
}
```

We're showing off another feature of the `query!`/`filter!` macro here
too. We look for the post(s) to be deleted based on title pattern. The
`like` method-like invocation on `title` transforms into the SQL LIKE
operator. So if we named our first post "First post" we could match it
with e.g. `First%`.

But what about the braces in `{ pattern }`? Normally names within the
macro refer to database columns/operators. The braces escape us back
to referring names in Rust code, so the value of the `pattern`
variable is used as the right-hand side for the `LIKE` operator.

If you delete a post, you can run `show_posts` again to confirm that it is fact deleted.

## Migrate

At some point we'll need to expand our models. Let's say we decide to
add a feature to allow visitors to "like" posts. Now we need to add a
`likes` field to `Post`. Let's go ahead and add

``` rust
pub likes: i32,
```

making the full model

``` rust
#[model]
pub struct Post {
    pub id: AutoPk<i32>,
    pub title: String,
    pub body: String,
    pub published: bool,
    pub tags: Many<Tag>,
    pub blog: ForeignKey<Blog>,
    pub byline: Option<String>,
    pub likes: i32,
    state: butane::ObjectState,
}
impl Post {
    pub fn new(blog: &Blog, title: String, body: String) -> Self {
        Post {
            id: -1,
            title,
            body,
            published: false,
            tags: Many::default(),
            blog: blog.into(),
            byline: None,
            likes: 0,
            state: ObjectState::default(),
        }
    }
}
```

Now we have to update our database. To do that, we create a new migration, we'll call this one "likes".

``` shell
cargo build
butane makemigration likes
```

And then apply it

``` shell
butane migrate
```

And that's it! Now we can use our new field.

## Embedding migrations

So far, the migrations are stored on the file-system.
If the application is distributed as an executable, those migrations need to be included.

To do that, the `embed` command will create a Rust file `butane_migrations.rs` which can be included into the code.

``` shell
butane embed
```

After running the `embed` sub-command, the generated code needs to be included into the Rust code.

Edit the `lib.rs` and add the following at the top:

``` rust
pub mod butane_migrations;
```

Opening the generated `butane_migrations.rs` shows it includes the same SQLite migrations that are stored on disk.

Now compiling the code will include the migrations, however we need to update the function `establish_connection`
to use these migrations:

``` rust
pub fn establish_connection() -> Connection {
    use butane::migrations::{Migration, Migrations};

    let mut connection = butane::db::connect(&ConnectionSpec::load(".butane/connection.json").unwrap()).unwrap();
    let migrations = butane_migrations::get_migrations().unwrap();
    let to_apply = migrations.unapplied_migrations(&connection).unwrap();
    for migration in to_apply {
        migration.apply(&mut connection).unwrap();
    }
    connection
}
```

Now the executable can automatically migrate a database to the current schema that executable requires.

## Adding PostgreSQL support

To add the PostgreSQL backend, run:

``` shell
butane backend add pg
```

The file-system migrations will be updated to include PostgreSQL support, and `butane_migrations.rs`
will also be updated to include the PostgreSQL migration scripts.

## Summary

While there are lots of aspects of Butane not covered in this
tutorial, hopefully it's conveyed an idea of how to get started. More
details can be found in the API docs.

[`butane::DataResult`]: https://docs.rs/butane/0.1.0/butane/trait.DataResult.html
[`butane::DataObject`]: https://docs.rs/butane/0.1.0/butane/trait.DataObject.html
[`save`]: https://docs.rs/butane/0.1.0/butane/trait.DataObject.html#tymethod.save
