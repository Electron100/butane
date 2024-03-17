#![allow(clippy::expect_fun_call)]
use std::env::args;

use butane::prelude::*;
use getting_started::models::Post;
use getting_started::*;

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
        .unwrap_or_else(|_| panic!("Unable to find post {id}"));
    // Just a normal Rust assignment, no fancy set methods
    post.published = true;
    post.save(&conn).await.unwrap();
    println!("Published post {}", post.title);
}
