use newtype::models::Blog;

#[test]
fn blog_name() {
    Blog::new("Dog").unwrap();
    Blog::new("Dog£").unwrap_err();
}
