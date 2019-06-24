use failure;
use propane::db::{BackendConnection, Connection, ConnectionSpec};
use propane::model;

use propane::prelude::*;

//temp for testing
use propane::query::{BoolExpr, Expr, Query};

pub type Result<T> = std::result::Result<T, failure::Error>;

#[model]
struct Blog {
    id: i64,
    name: String,
}

#[model]
struct Post {
    id: i64,
    title: String,
    body: String,
    published: bool,
    likes: i32,
    // TODO support foreign key
    //tags: ManyToMany<Tag>,
    //blog: ForeignKey<Blog>,
}

fn published_posts(conn: &impl BackendConnection) -> Result<Vec<Post>> {
    Post::query()
        .filter(Post::fieldexpr_published().eq(true))
        .load(conn)
}

/*
// TODO support pk attribute
#[model]
struct Tag {
    #[pk]
    tag: String,
}
*/

fn query() -> Result<()> {
    let conn = establish_connection()?;
    let _specific_post = Post::get(&conn, 1);
    let published_posts = query!(Post, published = true).limit(5).load(&conn);
    Ok(())
    /*
    let published_posts = Post::objects().where!(published = true).limit(5);
        let tagged_posts = Post::objects().where!(tags.contains("dinosaurs"));
        let tagged_posts2 = Post::objects().where!(tags.contains(tag = "dinosaurs"));
        let blog = Blog::objects.find!(name = "Bears").expect();
        let posts_in_blog = Post::objects().where!(blog = {blog})]
    */
}

fn establish_connection() -> Result<Connection> {
    let spec = ConnectionSpec::load(&std::env::current_dir()?)?;
    let conn = propane::db::connect(&spec)?;
    Ok(conn)
}
fn main() {
    println!("Hello, world!");
}
