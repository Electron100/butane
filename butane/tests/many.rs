use butane::db::Connection;
use butane::prelude::*;
use butane::{model, query::OrderDirection, Many, ObjectState};

use butane_test_helper::*;

mod common;
use common::blog::{create_tag, Blog, Post, Tag};

#[model]
#[derive(Clone)]
struct AutoPkWithMany {
    #[auto]
    id: i64,
    tags: Many<Tag>,
    items: Many<AutoItem>,
}
impl AutoPkWithMany {
    fn new() -> Self {
        AutoPkWithMany {
            id: -1,
            tags: Many::default(),
            items: Many::default(),
            state: ObjectState::default(),
        }
    }
}

#[model]
#[table = "renamed_many_table"]
#[derive(Clone)]
struct RenamedAutoPkWithMany {
    #[auto]
    id: i64,
    tags: Many<Tag>,
    items: Many<AutoItem>,
}
impl RenamedAutoPkWithMany {
    fn new() -> Self {
        RenamedAutoPkWithMany {
            id: -1,
            tags: Many::default(),
            items: Many::default(),
            state: ObjectState::default(),
        }
    }
}

#[model]
#[derive(Clone)]
struct AutoItem {
    #[auto]
    id: i64,
    val: String,
}

fn load_sorted_from_many(conn: Connection) {
    let mut cats_blog = Blog::new(1, "Cats");
    cats_blog.save(&conn).unwrap();
    let mut post = Post::new(
        1,
        "The Cheetah",
        "This post is about a fast cat.",
        &cats_blog,
    );
    let tag_fast = create_tag(&conn, "fast");
    let tag_cat = create_tag(&conn, "cat");
    let tag_european = create_tag(&conn, "european");

    post.tags.add(&tag_fast).unwrap();
    post.tags.add(&tag_cat).unwrap();
    post.tags.add(&tag_european).unwrap();
    post.save(&conn).unwrap();

    let post2 = Post::get(&conn, post.id).unwrap();
    let mut tag_iter = post2
        .tags
        .load_ordered(&conn, OrderDirection::Ascending)
        .unwrap();
    assert_eq!(tag_iter.next().unwrap().tag, "cat");
    assert_eq!(tag_iter.next().unwrap().tag, "european");
    assert_eq!(tag_iter.next().unwrap().tag, "fast");

    let post3 = Post::get(&conn, post.id).unwrap();
    let mut tag_iter = post3
        .tags
        .load_ordered(&conn, OrderDirection::Descending)
        .unwrap();
    assert_eq!(tag_iter.next().unwrap().tag, "fast");
    assert_eq!(tag_iter.next().unwrap().tag, "european");
    assert_eq!(tag_iter.next().unwrap().tag, "cat");
}
testall!(load_sorted_from_many);

fn remove_one_from_many(conn: Connection) {
    let mut cats_blog = Blog::new(1, "Cats");
    cats_blog.save(&conn).unwrap();
    let mut post = Post::new(
        1,
        "The Cheetah",
        "This post is about a fast cat.",
        &cats_blog,
    );
    let tag_fast = create_tag(&conn, "fast");
    let tag_cat = create_tag(&conn, "cat");
    let tag_european = create_tag(&conn, "european");

    post.tags.add(&tag_fast).unwrap();
    post.tags.add(&tag_cat).unwrap();
    post.tags.add(&tag_european).unwrap();
    post.save(&conn).unwrap();

    // Wait a minute, Cheetahs aren't from Europe!
    post.tags.remove(&tag_european);
    post.save(&conn).unwrap();

    let post2 = Post::get(&conn, post.id).unwrap();
    assert_eq!(post2.tags.load(&conn).unwrap().count(), 2);
}
testall!(remove_one_from_many);

fn remove_multiple_from_many(conn: Connection) {
    let mut cats_blog = Blog::new(1, "Cats");
    cats_blog.save(&conn).unwrap();
    let mut post = Post::new(
        1,
        "The Cheetah",
        "This post is about a fast cat.",
        &cats_blog,
    );
    let tag_fast = create_tag(&conn, "fast");
    let tag_cat = create_tag(&conn, "cat");
    let tag_european = create_tag(&conn, "european");
    let tag_striped = create_tag(&conn, "striped");

    post.tags.add(&tag_fast).unwrap();
    post.tags.add(&tag_cat).unwrap();
    post.tags.add(&tag_european).unwrap();
    post.tags.add(&tag_striped).unwrap();
    post.save(&conn).unwrap();

    // Wait a minute, Cheetahs aren't from Europe and they don't have stripes!
    post.tags.remove(&tag_european);
    post.tags.remove(&tag_striped);
    post.save(&conn).unwrap();

    let post2 = Post::get(&conn, post.id).unwrap();
    assert_eq!(post2.tags.load(&conn).unwrap().count(), 2);
}
testall!(remove_multiple_from_many);

fn can_add_to_many_before_save(conn: Connection) {
    // Verify that for an object with an auto-pk, we can add items to a Many field before we actually
    // save the original object (and thus get the actual pk);
    let mut obj = AutoPkWithMany::new();
    obj.tags.add(&create_tag(&conn, "blue")).unwrap();
    obj.tags.add(&create_tag(&conn, "red")).unwrap();
    obj.save(&conn).unwrap();

    let obj = AutoPkWithMany::get(&conn, obj.id).unwrap();
    let tags = obj.tags.load(&conn).unwrap();
    assert_eq!(tags.count(), 2);
}
testall!(can_add_to_many_before_save);

#[cfg(not(feature = "auto-save-related"))]
fn cant_add_unsaved_to_many(_conn: Connection) {
    let unsaved_item = AutoItem {
        id: -1,
        val: "shiny".to_string(),
        state: ObjectState::default(),
    };
    let mut obj = AutoPkWithMany::new();
    obj.items
        .add(&unsaved_item)
        .expect_err("unexpectedly not error");
}
#[cfg(not(feature = "auto-save-related"))]
testall!(cant_add_unsaved_to_many);

fn can_add_to_many_with_custom_table_name(conn: Connection) {
    let mut obj = RenamedAutoPkWithMany::new();
    obj.tags.add(&create_tag(&conn, "blue")).unwrap();
    obj.tags.add(&create_tag(&conn, "red")).unwrap();
    obj.save(&conn).unwrap();

    let obj = RenamedAutoPkWithMany::get(&conn, obj.id).unwrap();
    let tags = obj.tags.load(&conn).unwrap();
    assert_eq!(tags.count(), 2);
}
testall!(can_add_to_many_with_custom_table_name);
