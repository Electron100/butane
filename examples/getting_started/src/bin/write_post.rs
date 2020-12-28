use getting_started::*;
use models::Blog;
use butane::prelude::*;
use std::io::{stdin, Read};

fn main() {
    let conn = establish_connection();

    let blog = match Blog::query()
        .limit(1)
        .load(&conn)
        .unwrap()
        .into_iter()
        .last()
    {
        Some(blog) => blog,
        None => {
            println!("Enter blog name");
            let name = readline();
            create_blog(&conn, name)
        }
    };

    println!("Enter post title");
    let title = readline();
    println!("\nEnter text for {} (Press {} when finished)\n", title, EOF);
    let mut body = String::new();
    stdin().read_to_string(&mut body).unwrap();

    let post = create_post(&conn, &blog, title, body);
    println!("\nSaved draft {} with id {}", post.title, post.id);
}

fn readline() -> String {
    let mut s = String::new();
    stdin().read_line(&mut s).unwrap();
    // Drop the newline character
    s.pop();
    s
}

#[cfg(not(windows))]
const EOF: &str = "CTRL+D";

#[cfg(windows)]
const EOF: &str = "CTRL+Z";
