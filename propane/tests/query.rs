use paste;
use propane::db::Connection;
use propane::find;
use propane::prelude::*;
use propane::query;

mod common;
use common::blog;
use common::blog::{Blog, Post, Tag};

fn equality(conn: Connection) {
    blog::setup_blog(&conn);
    let mut posts = query!(Post, published == true).load(&conn).unwrap();
    assert_eq!(posts.len(), 3);
    posts.sort_by(|p1, p2| p1.id.partial_cmp(&p2.id).unwrap());
    assert_eq!(posts[0].title, "The Tiger");
    assert_eq!(posts[1].title, "Sir Charles");
    assert_eq!(posts[2].title, "Mount Doom");
}
testall!(equality);

fn comparison(conn: Connection) {
    blog::setup_blog(&conn);
    let mut posts = query!(Post, likes < 5).load(&conn).unwrap();
    assert_eq!(posts.len(), 2);
    posts.sort_by(|p1, p2| p1.id.partial_cmp(&p2.id).unwrap());
    assert_eq!(posts[0].title, "The Tiger");
    assert_eq!(posts[1].title, "Mt. Everest");
}
testall!(comparison);

fn combination(conn: Connection) {
    blog::setup_blog(&conn);
    let posts = query!(Post, published == true && likes < 5)
        .load(&conn)
        .unwrap();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].title, "The Tiger");
}
testall!(combination);

fn not_found(conn: Connection) {
    blog::setup_blog(&conn);
    let posts = query!(Post, published == false && likes > 5)
        .load(&conn)
        .unwrap();
    assert_eq!(posts.len(), 0);
}
testall!(not_found);

fn rustval(conn: Connection) {
    blog::setup_blog(&conn);
    // We don't need to escape into rust for this, but we can
    let post = find!(Post, title == { "The Tiger" }, &conn).unwrap();
    assert_eq!(post.title, "The Tiger");

    // or invoke a function that returns a value
    let f = || "The Tiger";
    let post2 = find!(Post, title == { f() }, &conn).unwrap();
    assert_eq!(post, post2);
}
testall!(rustval);

fn fkey_match(conn: Connection) {
    blog::setup_blog(&conn);
    let blog: Blog = find!(Blog, name == "Cats", &conn).unwrap();
    let mut posts = query!(Post, blog == { &blog }).load(&conn).unwrap();
    let posts2 = query!(Post, blog == { blog }).load(&conn).unwrap();
    let posts3 = query!(Post, blog.matches(name == "Cats"))
        .load(&conn)
        .unwrap();

    assert_eq!(posts.len(), 2);
    posts.sort_by(|p1, p2| p1.id.partial_cmp(&p2.id).unwrap());
    assert_eq!(posts[0].title, "The Tiger");
    assert_eq!(posts[1].title, "Sir Charles");
    assert_eq!(posts, posts2);
    assert_eq!(posts, posts3);
}
testall!(fkey_match);

fn many_load(conn: Connection) {
    blog::setup_blog(&conn);
    let post: Post = find!(Post, title == "The Tiger", &conn).unwrap();
    let tags = post.tags.load(&conn).unwrap();
    let mut tags: Vec<&Tag> = tags.collect();
    tags.sort_by(|t1, t2| (*t1).tag.partial_cmp(&t2.tag).unwrap());
    assert_eq!(tags[0].tag, "asia");
    assert_eq!(tags[1].tag, "danger");
}
testall!(many_load);

fn many_objects_with_tag(conn: Connection) {
    blog::setup_blog(&conn);
    let mut posts = query!(Post, tags.contains("danger")).load(&conn).unwrap();
    posts.sort_by(|p1, p2| p1.id.partial_cmp(&p2.id).unwrap());
    assert_eq!(posts[0].title, "The Tiger");
    assert_eq!(posts[1].title, "Mount Doom");
    assert_eq!(posts[2].title, "Mt. Everest");
}
testall!(many_objects_with_tag);

fn many_objects_with_tag_explicit(conn: Connection) {
    blog::setup_blog(&conn);
    let mut posts = query!(Post, tags.contains(tag == "danger"))
        .load(&conn)
        .unwrap();
    posts.sort_by(|p1, p2| p1.id.partial_cmp(&p2.id).unwrap());
    assert_eq!(posts[0].title, "The Tiger");
    assert_eq!(posts[1].title, "Mount Doom");
    assert_eq!(posts[2].title, "Mt. Everest");
}
testall!(many_objects_with_tag_explicit);
