use propane::model;

#[model]
#[derive(Default)]
pub struct Post {
    #[auto]
    pub id: i32,
    pub title: String,
    pub body: String,
    pub published: bool,
}
impl Post {
    pub fn new(title: String, body: String) -> Self {
        Post {
            title,
            body,
            ..Default::default()
        }
    }
}
