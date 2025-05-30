use butane::{model, query::OrderDirection, AutoPk, Many};
use butane_test_helper::*;
use butane_test_macros::butane_test;

mod common;
use common::blog::{create_tag, create_tag_sync, Blog, Post, Tag};

#[model]
struct AutoPkWithMany {
    id: AutoPk<i64>,
    tags: Many<Tag>,
    items: Many<AutoItem>,
}
impl AutoPkWithMany {
    fn new() -> Self {
        AutoPkWithMany {
            id: AutoPk::uninitialized(),
            tags: Many::default(),
            items: Many::default(),
        }
    }
}

#[model]
#[table = "renamed_many_table"]
struct RenamedAutoPkWithMany {
    id: AutoPk<i64>,
    tags: Many<Tag>,
    items: Many<AutoItem>,
}
impl RenamedAutoPkWithMany {
    fn new() -> Self {
        RenamedAutoPkWithMany {
            id: AutoPk::uninitialized(),
            tags: Many::default(),
            items: Many::default(),
        }
    }
}

#[model]
struct AutoItem {
    id: AutoPk<i64>,
    val: String,
}

#[butane_test]
async fn load_sorted_from_many(conn: ConnectionAsync) {
    let mut cats_blog = Blog::new(1, "Cats");
    cats_blog.save(&conn).await.unwrap();
    let mut post = Post::new(
        1,
        "The Cheetah",
        "This post is about a fast cat.",
        &cats_blog,
    );
    post.save(&conn).await.unwrap();

    let tag_fast = create_tag(&conn, "fast").await;
    let tag_cat = create_tag(&conn, "cat").await;
    let tag_european = create_tag(&conn, "european").await;
    let tags = vec![tag_fast, tag_cat, tag_european];

    post.tags.set(&conn, tags).await.unwrap();

    let saved_tags: Vec<&Tag> = post.tags.get().unwrap().into_iter().collect();
    assert_eq!(saved_tags.len(), 3);
    assert_eq!(saved_tags[0].tag, "fast");
    assert_eq!(saved_tags[1].tag, "cat");
    assert_eq!(saved_tags[2].tag, "european");

    let post2 = Post::get(&conn, post.id).await.unwrap();
    let mut tag_iter = post2
        .tags
        .load_ordered(&conn, OrderDirection::Ascending)
        .await
        .unwrap();
    assert_eq!(tag_iter.next().unwrap().tag, "cat");
    assert_eq!(tag_iter.next().unwrap().tag, "european");
    assert_eq!(tag_iter.next().unwrap().tag, "fast");
    assert!(tag_iter.next().is_none());

    let post3 = Post::get(&conn, post.id).await.unwrap();
    let mut tag_iter = post3
        .tags
        .load_ordered(&conn, OrderDirection::Descending)
        .await
        .unwrap();
    assert_eq!(tag_iter.next().unwrap().tag, "fast");
    assert_eq!(tag_iter.next().unwrap().tag, "european");
    assert_eq!(tag_iter.next().unwrap().tag, "cat");
    assert!(tag_iter.next().is_none());
}

#[butane_test]
async fn remove_one_from_many(conn: ConnectionAsync) {
    let mut cats_blog = Blog::new(1, "Cats");
    cats_blog.save(&conn).await.unwrap();
    let mut post = Post::new(
        1,
        "The Cheetah",
        "This post is about a fast cat.",
        &cats_blog,
    );
    let tag_fast = create_tag(&conn, "fast").await;
    let tag_cat = create_tag(&conn, "cat").await;
    let tag_european = create_tag(&conn, "european").await;

    post.tags.add(&tag_fast).unwrap();
    post.tags.add(&tag_cat).unwrap();
    post.tags.add(&tag_european).unwrap();
    post.save(&conn).await.unwrap();

    // Wait a minute, Cheetahs aren't from Europe!
    post.tags.remove(&tag_european);
    post.save(&conn).await.unwrap();

    let post2 = Post::get(&conn, post.id).await.unwrap();
    assert_eq!(post2.tags.load(&conn).await.unwrap().count(), 2);
}

#[butane_test]
async fn remove_multiple_from_many(conn: ConnectionAsync) {
    let mut cats_blog = Blog::new(1, "Cats");
    cats_blog.save(&conn).await.unwrap();
    let mut post = Post::new(
        1,
        "The Cheetah",
        "This post is about a fast cat.",
        &cats_blog,
    );
    let tag_fast = create_tag(&conn, "fast").await;
    let tag_cat = create_tag(&conn, "cat").await;
    let tag_european = create_tag(&conn, "european").await;
    let tag_striped = create_tag(&conn, "striped").await;

    post.tags.add(&tag_fast).unwrap();
    post.tags.add(&tag_cat).unwrap();
    post.tags.add(&tag_european).unwrap();
    post.tags.add(&tag_striped).unwrap();
    post.save(&conn).await.unwrap();

    // Wait a minute, Cheetahs aren't from Europe and they don't have stripes!
    post.tags.remove(&tag_european);
    post.tags.remove(&tag_striped);
    post.save(&conn).await.unwrap();

    let post2 = Post::get(&conn, post.id).await.unwrap();
    assert_eq!(post2.tags.load(&conn).await.unwrap().count(), 2);
}

#[butane_test]
async fn delete_all_from_many(conn: ConnectionAsync) {
    let mut cats_blog = Blog::new(1, "Cats");
    cats_blog.save(&conn).await.unwrap();
    let mut post = Post::new(
        1,
        "The Cheetah",
        "This post is about a fast cat.",
        &cats_blog,
    );
    let tag_fast = create_tag(&conn, "fast").await;
    let tag_cat = create_tag(&conn, "cat").await;
    let tag_european = create_tag(&conn, "european").await;
    let tag_striped = create_tag(&conn, "striped").await;

    post.tags.add(&tag_fast).unwrap();
    post.tags.add(&tag_cat).unwrap();
    post.tags.add(&tag_european).unwrap();
    post.save(&conn).await.unwrap();
    post.tags.add(&tag_striped).unwrap();

    post.tags.delete(&conn).await.unwrap();

    // Verify that the tags are deleted from the Many object
    let tags: Vec<&Tag> = post.tags.get().unwrap().collect();
    assert_eq!(tags.len(), 0);

    // Verify that the tags are deleted from the database
    let post2 = Post::get(&conn, post.id).await.unwrap();
    assert_eq!(post2.tags.load(&conn).await.unwrap().count(), 0);
}

#[butane_test]
async fn can_add_to_many_before_save(conn: ConnectionAsync) {
    // Verify that for an object with an auto-pk, we can add items to a Many field before we actually
    // save the original object (and thus get the actual pk);
    let mut obj = AutoPkWithMany::new();
    obj.tags.add(&create_tag(&conn, "blue").await).unwrap();
    obj.tags.add(&create_tag(&conn, "red").await).unwrap();
    obj.save(&conn).await.unwrap();

    let obj = AutoPkWithMany::get(&conn, obj.id).await.unwrap();
    let tags = obj.tags.load(&conn).await.unwrap();
    assert_eq!(tags.count(), 2);
}

#[butane_test]
async fn cant_add_unsaved_to_many(_conn: ConnectionAsync) {
    let unsaved_item = AutoItem {
        id: AutoPk::uninitialized(),
        val: "shiny".to_string(),
    };
    let mut obj = AutoPkWithMany::new();
    let err = obj
        .items
        .add(&unsaved_item)
        .expect_err("unexpectedly not error");
    assert!(matches!(err, butane::Error::ValueNotSaved));
}

#[butane_test]
async fn can_add_to_many_with_custom_table_name(conn: ConnectionAsync) {
    let mut obj = RenamedAutoPkWithMany::new();
    obj.tags.add(&create_tag(&conn, "blue").await).unwrap();
    obj.tags.add(&create_tag(&conn, "red").await).unwrap();
    obj.save(&conn).await.unwrap();

    let obj = RenamedAutoPkWithMany::get(&conn, obj.id).await.unwrap();
    let tags = obj.tags.load(&conn).await.unwrap();
    assert_eq!(tags.count(), 2);
}
