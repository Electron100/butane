use butane::db::{Connection, ConnectionAsync};
use butane::{find, find_async, ForeignKey};
use butane_test_helper::*;
use butane_test_macros::butane_test;
use fake::{Fake, Faker};

mod common;
use common::blog::{Blog, Post, Tag};

#[butane_test]
async fn fake_blog_post(conn: ConnectionAsync) {
    let mut fake_blog: Blog = Faker.fake();
    fake_blog.save(&conn).await.unwrap();

    let mut post: Post = Faker.fake();
    post.blog = ForeignKey::<Blog>::from(fake_blog);

    let mut tag_1: Tag = Faker.fake();
    tag_1.save(&conn).await.unwrap();
    let mut tag_2: Tag = Faker.fake();
    tag_2.save(&conn).await.unwrap();
    let mut tag_3: Tag = Faker.fake();
    tag_3.save(&conn).await.unwrap();

    post.tags.add(&tag_1).unwrap();
    post.tags.add(&tag_2).unwrap();
    post.tags.add(&tag_3).unwrap();
    post.save(&conn).await.unwrap();

    let post_from_db = find_async!(Post, id == { post.id }, &conn).unwrap();
    assert_eq!(post_from_db.title, post.title);
    assert_eq!(post_from_db.tags.load(&conn).await.unwrap().count(), 3);
}
