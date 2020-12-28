#![allow(clippy::expect_fun_call)]
use self::models::Post;
use getting_started::*;
use butane::prelude::*;
use std::env::args;

fn main() {
    let id = args()
        .nth(1)
        .expect("publish_post requires a post id")
        .parse::<i32>()
        .expect("Invalid ID");
    let conn = establish_connection();

    let mut post = Post::get(&conn, id).expect(&format!("Unable to find post {}", id));
    // Just a normal Rust assignment, no fancy set methods
    post.published = true;
    post.save(&conn).expect("Unable to update post");
    println!("Published post {}", post.title);
}
