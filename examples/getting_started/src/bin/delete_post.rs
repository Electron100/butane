use self::models::Post;
use getting_started::*;
use propane::query;
use std::env::args;

fn main() {
    let target = args().nth(1).expect("Expected a target to match against");
    let pattern = format!("%{}%", target);

    let conn = establish_connection();
    // TODO count deleted posts and print number
    query!(Post, title.like({ pattern }))
        .delete(&conn)
        .expect("error deleting posts");
}
