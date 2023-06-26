use getting_started::*;
use std::io::{stdin, Read};

fn main() {
    let conn = establish_connection();

    let blog = match existing_blog(&conn) {
        Some(blog) => blog,
        None => {
            println!("Enter blog name");
            let name = readline();
            create_blog(&conn, name)
        }
    };

    println!("Enter post title");
    let title = readline();
    println!("\nEnter text for {title} ({EOF} when finished)\n");
    let mut body = String::new();
    stdin().read_to_string(&mut body).unwrap();

    let post = create_post(&conn, blog, title, body);
    println!(
        "\nSaved unpublished post {} with id {}",
        post.title, post.id
    );
}

fn readline() -> String {
    let mut s = String::new();
    stdin().read_line(&mut s).unwrap();
    s.pop(); // Drop the newline
    s
}

#[cfg(not(windows))]
const EOF: &str = "CTRL+D";

#[cfg(windows)]
const EOF: &str = "CTRL+Z";
