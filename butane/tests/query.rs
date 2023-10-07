use butane::db::Connection;
use butane::prelude::*;
use butane::query::BoolExpr;
use butane::{colname, filter, find, query, Many};
use chrono::{TimeZone, Utc};

use butane_test_helper::*;

mod common;
use common::blog;
use common::blog::{Blog, Post, PostMetadata, Tag};

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

fn equality_separate_dataresult(conn: Connection) {
    blog::setup_blog(&conn);
    let mut posts = query!(PostMetadata, published == true).load(&conn).unwrap();
    assert_eq!(posts.len(), 3);
    posts.sort_by(|p1, p2| p1.id.partial_cmp(&p2.id).unwrap());
    assert_eq!(posts[0].title, "The Tiger");
    assert_eq!(posts[1].title, "Sir Charles");
    assert_eq!(posts[2].title, "Mount Doom");
}
testall!(equality_separate_dataresult);

fn ordered(conn: Connection) {
    blog::setup_blog(&conn);
    let posts = query!(Post, published == true)
        .order_asc(colname!(Post, title))
        .load(&conn)
        .unwrap();
    assert_eq!(posts.len(), 3);
    assert_eq!(posts[0].title, "Mount Doom");
    assert_eq!(posts[1].title, "Sir Charles");
    assert_eq!(posts[2].title, "The Tiger");
}
testall!(ordered);

fn comparison(conn: Connection) {
    blog::setup_blog(&conn);
    let mut posts = query!(Post, likes < 5).load(&conn).unwrap();
    assert_eq!(posts.len(), 2);
    posts.sort_by(|p1, p2| p1.id.partial_cmp(&p2.id).unwrap());
    assert_eq!(posts[0].title, "The Tiger");
    assert_eq!(posts[1].title, "Mt. Everest");
}
testall!(comparison);

fn like(conn: Connection) {
    blog::setup_blog(&conn);
    let mut posts = query!(Post, title.like("M%")).load(&conn).unwrap();
    assert_eq!(posts.len(), 2);
    posts.sort_by(|p1, p2| p1.id.partial_cmp(&p2.id).unwrap());
    assert_eq!(posts[0].title, "Mount Doom");
    assert_eq!(posts[1].title, "Mt. Everest");
}
testall!(like);

fn combination(conn: Connection) {
    blog::setup_blog(&conn);
    let posts = query!(Post, published == true && likes < 5)
        .load(&conn)
        .unwrap();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].title, "The Tiger");
}
testall!(combination);

fn combination_allof(conn: Connection) {
    blog::setup_blog(&conn);
    let posts = Post::query()
        .filter(BoolExpr::AllOf(vec![
            filter!(Post, published == true),
            filter!(Post, likes < 5),
            filter!(Post, title == "The Tiger"),
        ]))
        .load(&conn)
        .unwrap();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].title, "The Tiger");
}
testall!(combination_allof);

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
    let blog_id = blog.id;
    let posts3 = query!(Post, blog == { blog_id }).load(&conn).unwrap();
    let posts4 = query!(Post, blog.matches(name == "Cats"))
        .load(&conn)
        .unwrap();

    assert_eq!(posts.len(), 2);
    posts.sort_by(|p1, p2| p1.id.partial_cmp(&p2.id).unwrap());
    assert_eq!(posts[0].title, "The Tiger");
    assert_eq!(posts[1].title, "Sir Charles");
    assert_eq!(posts, posts2);
    assert_eq!(posts, posts3);
    assert_eq!(posts, posts4);
}
testall!(fkey_match);

fn many_load(conn: Connection) {
    blog::setup_blog(&conn);
    let post: Post = find!(Post, title == "The Tiger", &conn).unwrap();
    let tags = post.tags.load(&conn).unwrap();
    let mut tags: Vec<&Tag> = tags.collect();
    tags.sort_by(|t1, t2| t1.tag.partial_cmp(&t2.tag).unwrap());
    assert_eq!(tags[0].tag, "asia");
    assert_eq!(tags[1].tag, "danger");
}
testall!(many_load);

fn many_serialize(conn: Connection) {
    blog::setup_blog(&conn);
    let post: Post = find!(Post, title == "The Tiger", &conn).unwrap();
    let tags_json: String = serde_json::to_string(&post.tags).unwrap();
    let tags: Many<Tag> = serde_json::from_str(&tags_json).unwrap();
    let tags = tags.load(&conn).unwrap();
    let mut tags: Vec<&Tag> = tags.collect();
    tags.sort_by(|t1, t2| t1.tag.partial_cmp(&t2.tag).unwrap());
    assert_eq!(tags[0].tag, "asia");
    assert_eq!(tags[1].tag, "danger");
}
testall!(many_serialize);

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

fn by_timestamp(conn: Connection) {
    blog::setup_blog(&conn);
    let mut post = find!(Post, title == "Sir Charles", &conn).unwrap();
    // Pretend this post was published in 1970
    post.pub_time = Some(
        Utc.with_ymd_and_hms(1970, 1, 1, 1, 1, 1)
            .single()
            .unwrap()
            .naive_utc(),
    );
    eprintln!("Starting save");
    post.save(&conn).unwrap();
    eprintln!("Stopping save");
    // And pretend another post was later in 1971
    let mut post = find!(Post, title == "The Tiger", &conn).unwrap();
    post.pub_time = Some(
        Utc.with_ymd_and_hms(1970, 5, 1, 1, 1, 1)
            .single()
            .unwrap()
            .naive_utc(),
    );
    post.save(&conn).unwrap();

    // Now find all posts published before 1971. Assume we haven't gone
    // back in time to run these unit tests.
    let posts = query!(
        Post,
        pub_time < {
            Utc.with_ymd_and_hms(1972, 1, 1, 1, 1, 1)
                .single()
                .unwrap()
                .naive_utc()
        }
    )
    .order_desc(colname!(Post, pub_time))
    .load(&conn)
    .unwrap();
    assert_eq!(posts[0].title, "The Tiger");
    assert_eq!(posts[1].title, "Sir Charles");
}
testall!(by_timestamp);

fn limit(conn: Connection) {
    blog::setup_blog(&conn);
    let posts = Post::query()
        .order_asc(colname!(Post, title))
        .limit(2)
        .load(&conn)
        .unwrap();
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].title, "Mount Doom");
    assert_eq!(posts[1].title, "Mt. Everest");
}
testall!(limit);

fn offset(conn: Connection) {
    blog::setup_blog(&conn);
    // Now get the more posts after the two we got in the limit test above
    let posts = Post::query()
        .order_asc(colname!(Post, title))
        .offset(2)
        .load(&conn)
        .unwrap();
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].title, "Sir Charles");
    assert_eq!(posts[1].title, "The Tiger");
}
testall!(offset);
