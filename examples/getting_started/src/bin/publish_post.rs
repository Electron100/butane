#![allow(clippy::expect_fun_call)]
use self::models::Post;
use butane::prelude::*;
use getting_started::*;
use std::env::args;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let id = args()
        .nth(1)
        .expect("publish_post requires a post id")
        .parse::<i32>()
        .expect("Invalid ID");
    let conn = establish_connection().await;

    let mut post = Post::get(&conn, id)
        .await
        .expect(&format!("Unable to find post {id}"));
    // Just a normal Rust assignment, no fancy set methods
    post.published = true;
    post.save(&conn).await.unwrap();
    println!("Published post {}", post.title);
}
