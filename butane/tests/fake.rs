use butane::db::Connection;
use butane::{find, model, DataObject, ForeignKey, Many};
use butane_test_helper::*;

use fake::{Dummy, Fake, Faker};

mod common;
use common::blog::{Blog, Post, Tag};

fn fake_blog_post(conn: Connection) {
    let mut fake_blog: Blog = Faker.fake();
    fake_blog.save(&conn).unwrap();

    let mut post: Post = Faker.fake();
    post.blog = ForeignKey::<Blog>::from(fake_blog);

    let mut tag_1: Tag = Faker.fake();
    tag_1.save(&conn).unwrap();
    let mut tag_2: Tag = Faker.fake();
    tag_2.save(&conn).unwrap();
    let mut tag_3: Tag = Faker.fake();
    tag_3.save(&conn).unwrap();

    post.tags.add(&tag_1).unwrap();
    post.tags.add(&tag_2).unwrap();
    post.tags.add(&tag_3).unwrap();
    post.save(&conn).unwrap();

    let post_from_db = find!(Post, id == { post.id }, &conn).unwrap();
    assert_eq!(post_from_db.title, post.title);
    assert_eq!(post_from_db.tags.load(&conn).unwrap().count(), 3);
}
testall!(fake_blog_post);

// We dont want the main struct's to have `Clone`, lest
// that becomes necessary without being noticed.
#[model]
#[derive(Clone, Debug, Dummy)]
struct ClonableBlog {
    pub id: i64,
    pub name: String,
}

#[model]
#[derive(Clone, Debug, Dummy)]
struct ClonablePost {
    pub id: i64,
    pub title: String,
    pub tags: Many<ClonableTag>,
    pub blog: ForeignKey<ClonableBlog>,
}

#[model]
#[table = "tags"]
#[derive(Clone, Debug, Dummy)]
pub struct ClonableTag {
    #[pk]
    pub tag: String,
}

/// Fake ForeignKey values can be accessed, but will not be saved
/// resulting in inability to load them from the database.
fn fake_auto_relationship_values(conn: Connection) {
    let mut post: ClonablePost = Faker.fake();

    // The ForeignKey value can be accessed before being saved
    assert!(post.blog.get().is_ok());
    // The Many has a value in it
    assert_eq!(post.tags.get().unwrap().count(), 1);

    let mut blog: ClonableBlog = post.blog.get().unwrap().clone();
    blog.save(&conn).unwrap();
    let blog_name = post.blog.get().unwrap().name.clone();
    assert!(post.blog.load(&conn).is_ok());
    assert_eq!(post.blog.load(&conn).unwrap().name, blog_name);

    assert_ne!(post.blog.pk(), 0);

    let mut tag: ClonableTag = post.tags.get().unwrap().next().unwrap().clone();
    eprintln!("tag: {:?}", tag);
    tag.save(&conn).unwrap();

    let tag_from_db = find!(ClonableTag, tag == { tag.tag }, &conn).unwrap();
    assert_eq!(tag_from_db.tag, tag_from_db.tag);

    // With the Blog & Tag saved, we can save the Post,
    // however the Many<Tag> wont be saved because they are in `all_values`.
    post.save(&conn).unwrap();

    let post_from_db = find!(ClonablePost, id == { post.id }, &conn).unwrap();
    assert_eq!(post_from_db.title, post.title);

    assert!(post_from_db.blog.load(&conn).is_ok());
    assert_eq!(post_from_db.blog.load(&conn).unwrap().name, blog_name);

    // The Many<T> were not saved
    assert_eq!(post_from_db.tags.load(&conn).unwrap().count(), 0);
}
testall!(fake_auto_relationship_values);
