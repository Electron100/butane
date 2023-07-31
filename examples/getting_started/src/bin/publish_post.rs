#![allow(clippy::expect_fun_call)]
use std::env::args;

use butane::prelude::*;
use getting_started::*;

use self::models::Post;

fn main() {
    let id = args()
        .nth(1)
        .expect("publish_post requires a post id")
        .parse::<i32>()
        .expect("Invalid ID");
    let conn = establish_connection();

    let mut post = Post::get(&conn, id).unwrap_or_else(|_| panic!("Unable to find post {id}"));
    // Just a normal Rust assignment, no fancy set methods
    post.published = true;
    post.save(&conn).unwrap();
    println!("Published post {}", post.title);
}
