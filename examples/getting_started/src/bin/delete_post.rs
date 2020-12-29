use self::models::Post;
use butane::query;
use getting_started::*;
use std::env::args;

fn main() {
    let target = args().nth(1).expect("Expected a target to match against");
    let pattern = format!("%{}%", target);

    let conn = establish_connection();
    let cnt = query!(Post, title.like({ pattern }))
        .delete(&conn)
        .expect("error deleting posts");
    println!("Deleted {} posts", cnt);
}
