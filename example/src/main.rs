#![allow(dead_code)]
use butane::db::{Connection, ConnectionSpec};
use butane::model;
use butane::Error;
use butane::ObjectState;
use butane::{find, query};
use butane::{ForeignKey, Many};

use butane::prelude::*;

pub type Result<T> = std::result::Result<T, Error>;

#[model]
#[derive(Debug, Default)]
struct Blog {
    #[auto]
    id: i64,
    name: String,
}

#[model]
#[derive(Debug)]
struct Post {
    #[auto]
    id: i64,
    title: String,
    body: String,
    published: bool,
    likes: i32,
    tags: Many<Tag>,
    blog: ForeignKey<Blog>,
    byline: Option<String>,
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

#[model]
#[derive(Debug)]
struct Tag {
    #[pk]
    tag: String,
}

fn query() -> Result<()> {
    let conn = establish_connection()?;
    let mut blog = Blog {
        name: "Bears".into(),
        ..Default::default()
    };
    blog.save(&conn).unwrap();

    let mut post = Post::new(&blog, "Grizzly".into(), "lorem ipsum".into());
    post.published = true;
    post.save(&conn).unwrap();

    let _specific_post = Post::get(&conn, 1);
    let _published_posts = query!(Post, published == true).limit(5).load(&conn)?;
    let unliked_posts = query!(Post, published == true && likes < 5).load(&conn)?;
    let _blog: &Blog = unliked_posts.first().unwrap().blog.load(&conn)?;
    let _tagged_posts = query!(Post, tags.contains("dinosaurs")).load(&conn);
    //let tagged_posts2 = query!(Post, tags.contains(tag == "dinosaurs")).load(&conn);
    let blog: Blog = find!(Blog, name == "Bears", &conn).unwrap();
    let _posts_in_blog = query!(Post, blog == { &blog }).load(&conn);
    let _posts_in_blog2 = query!(Post, blog == { blog }).load(&conn);
    let _posts_in_blog = query!(Post, blog.matches(name == "Bears")).load(&conn);
    Ok(())
}

fn establish_connection() -> Result<Connection> {
    let mut cwd = std::env::current_dir()?;
    cwd.push(".butane");
    let spec = ConnectionSpec::load(cwd)?;
    let conn = butane::db::connect(&spec)?;
    Ok(conn)
}
fn main() -> Result<()> {
    query()
}
