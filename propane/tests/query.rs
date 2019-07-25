use paste;
use propane::db::{Connection, ConnectionSpec};
use propane::model;
use propane::prelude::*;
use propane::query;
use propane::ForeignKey;

mod common;
use common::blog;
use common::blog::Post;

fn sort_posts(posts: &mut Vec<Post>) {}

fn published_posts(conn: Connection) {
    blog::setup_blog(&conn);
    let posts = query!(Post, published == true).load(&conn).unwrap();
    assert_eq!(posts.len(), 3);
    posts.sort_by(|p1, p2| p1.id.partial_cmp(p2.id));
    assert_eq!(posts[0].title, "The Tiger");
    assert_eq!(posts[1].title, "Sir Charles");
    assert_eq!(posts[2].title, "Mount Doom");
}
testall!(published_posts);
