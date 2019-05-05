use propane::model;

#[model]
struct Blog {
    id: i64,
    name: String,
}

#[model]
struct Post {
    id: i64,
    title: String,
    body: String,
    published: bool,
    //tags: ManyToMany<Tag>,
    //blog: ForeignKey<Blog>,
}

#[model]
struct Tag {
    //#[pk]
    tag: String,
}

/*
fn query() {
      let conn = establish_connection();
        let specific_post = Post::get(1);
        let published_posts = Post::objects().where!(published = true).limit(5);
        let tagged_posts = Post::objects().where!(tags.contains("dinosaurs"));
        let tagged_posts2 = Post::objects().where!(tags.contains(tag = "dinosaurs"));
        let blog = Blog::objects.find!(name = "Bears").expect();
        let posts_in_blog = Post::objects().where!(blog = {blog})
     }
*/
fn main() {
    println!("Hello, world!");
}
