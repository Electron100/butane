use butane::prelude_async::*;
use butane::query;
use getting_started_async::models::Post;
use getting_started_async::*;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let conn = establish_connection().await;
    let results = query!(Post, published == true)
        .limit(5)
        .load(&conn)
        .await
        .expect("Error loading posts");
    println!("Displaying {} posts", results.len());
    for post in results {
        println!("{} ({} likes)", post.title, post.likes);
        println!("----------\n");
        println!("{}", post.body);
    }
}
