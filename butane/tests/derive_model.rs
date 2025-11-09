//! Test that #[derive(DataObject)] works the same as #[model]

use butane::db::ConnectionAsync;
use butane::{query, AutoPk, DataObject};
use butane_test_helper::*;
use butane_test_macros::butane_test;

// Test basic derive(DataObject) usage
#[derive(DataObject, Debug, Clone, PartialEq)]
struct ProductDataObject {
    id: AutoPk<i64>,
    name: String,
    price: i32,
}

impl ProductDataObject {
    fn new(name: String, price: i32) -> Self {
        ProductDataObject {
            id: AutoPk::default(),
            name,
            price,
        }
    }
}

// Test derive(DataObject) with custom table name and pk
#[derive(DataObject, Debug, PartialEq)]
#[table = "items"]
struct ItemDataObject {
    #[pk]
    sku: String,
    description: String,
}

#[butane_test]
async fn basic_derive(conn: ConnectionAsync) {
    let mut product = ProductDataObject::new("Widget".to_string(), 100);
    product.save(&conn).await.unwrap();

    let loaded = ProductDataObject::get(&conn, product.id).await.unwrap();
    assert_eq!(product, loaded);
    assert_eq!(loaded.name, "Widget");
    assert_eq!(loaded.price, 100);
}

#[butane_test]
async fn derive_with_custom_table(conn: ConnectionAsync) {
    let mut item = ItemDataObject {
        sku: "ABC123".to_string(),
        description: "Test item".to_string(),
    };
    item.save(&conn).await.unwrap();

    let loaded = ItemDataObject::get(&conn, "ABC123".to_string())
        .await
        .unwrap();
    assert_eq!(item, loaded);
    assert_eq!(loaded.description, "Test item");
}

#[butane_test]
async fn derive_query(conn: ConnectionAsync) {
    let mut p1 = ProductDataObject::new("Cheap".to_string(), 50);
    let mut p2 = ProductDataObject::new("Expensive".to_string(), 200);
    p1.save(&conn).await.unwrap();
    p2.save(&conn).await.unwrap();

    let results = query!(ProductDataObject, price < 100)
        .load(&conn)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Cheap");
}
