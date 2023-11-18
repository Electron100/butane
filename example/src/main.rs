use butane::db::{Connection, ConnectionSpec};
use butane::prelude::*;
use butane::{find, model, query, AutoPk, Error, ForeignKey, Many};

type Result<T> = std::result::Result<T, Error>;

#[model]
#[derive(Debug, Default)]
struct Blog {
    id: AutoPk<i64>,
    name: String,
}

#[model]
#[derive(Debug)]
struct Post {
    id: AutoPk<i64>,
    title: String,
    body: String,
    published: bool,
    likes: i32,
    tags: Many<Tag>,
    blog: ForeignKey<Blog>,
    byline: Option<String>,
}
impl Post {
    fn new(blog: &Blog, title: String, body: String) -> Self {
        Post {
            id: AutoPk::default(),
            title,
            body,
            published: false,
            tags: Many::default(),
            blog: blog.into(),
            byline: None,
            likes: 0,
        }
    }
}

#[model]
#[derive(Debug, Default)]
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

    let mut tag = Tag {
        tag: "dinosaurs".into(),
    };
    tag.save(&conn).unwrap();

    let mut post = Post::new(&blog, "Grizzly".into(), "lorem ipsum".into());
    post.published = true;
    post.tags.add(&tag)?;
    post.save(&conn).unwrap();

    let _specific_post = Post::get(&conn, 1)?;
    let published_posts = query!(Post, published == true).limit(5).load(&conn)?;
    assert!(!published_posts.is_empty());
    let unliked_posts = query!(Post, published == true && likes < 5).load(&conn)?;
    assert!(!unliked_posts.is_empty());
    let _blog: &Blog = unliked_posts.first().unwrap().blog.load(&conn)?;
    let tagged_posts = query!(Post, tags.contains("dinosaurs")).load(&conn)?;
    assert!(!tagged_posts.is_empty());
    let tagged_posts = query!(Post, tags.contains(tag == "dinosaurs")).load(&conn)?;
    assert!(!tagged_posts.is_empty());
    let blog: Blog = find!(Blog, name == "Bears", &conn).unwrap();
    let posts_in_blog = query!(Post, blog == { &blog }).load(&conn)?;
    assert!(!posts_in_blog.is_empty());
    let posts_in_blog = query!(Post, blog == { blog }).load(&conn)?;
    assert!(!posts_in_blog.is_empty());
    let posts_in_blog = query!(Post, blog.matches(name == "Bears")).load(&conn)?;
    assert!(!posts_in_blog.is_empty());
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
