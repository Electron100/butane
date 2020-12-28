use getting_started::models::*;
use getting_started::*;
use butane::query;

fn main() {
    let conn = establish_connection();
    let results = query!(Post, published == true)
        .limit(5)
        .load(&conn)
        .expect("Error loading posts");
    println!("Displaying {} posts", results.len());
    for post in results {
        println!("{}", post.title);
        println!("----------\n");
        println!("{}", post.body);
    }
}
