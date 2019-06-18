use failure;
use propane::db::{BackendConnection, Connection, ConnectionSpec};
use propane::model;

use propane::prelude::*;

//temp for testing
use propane::adb::AType;
use propane::db::Column;
use propane::field::FieldExpr;
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
    //tags: ManyToMany<Tag>,
    //blog: ForeignKey<Blog>,
}

impl propane::DBObject for Post {
    type PKType = i64;
    const COLUMNS: &'static [Column] = &[
        Column::new("id", AType::BigInt),
        Column::new("title", AType::Text),
        Column::new("body", AType::Text),
        Column::new("published", AType::Bool),
        Column::new("likes", AType::Int),
    ];
    fn get(
        conn: &impl BackendConnection,
        id: Self::PKType,
    ) -> std::result::Result<Self, failure::Error> {
        Self::query()
            .filter(BoolExpr::Eq("id", Expr::Val(id.into())))
            .limit(1)
            .load(conn)?
            .into_iter()
            .nth(0)
            .ok_or(propane::Error::NoSuchObject.into())
    }
    fn query() -> Query {
        Query::new("Post")
    }
    fn from_row(mut row: propane::db::Row) -> propane::Result<Self> {
        Ok(Post {
            id: row.get_int(0)?,
            title: row.retrieve_text(1)?,
            body: row.retrieve_text(2)?,
            published: row.get_bool(3)?,
            likes: row.get_int(4)? as i32,
        })
    }
}

trait PostPropane {
    fn fieldexpr_id() -> FieldExpr<i64> {
        FieldExpr::<i64>::new("id")
    }
    fn fieldexpr_published() -> FieldExpr<bool> {
        FieldExpr::<bool>::new("published")
    }
}

impl PostPropane for Post {}

fn published_posts(conn: &impl BackendConnection) -> Result<Vec<Post>> {
    Post::query()
        .filter(<Post as PostPropane>::fieldexpr_published().eq(true))
        .load(conn)
}

#[model]
struct Tag {
    //#[pk]
    tag: String,
}

fn query() -> Result<()> {
    let conn = establish_connection()?;
    let _specific_post = Post::get(&conn, 1);
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
