use propane::migrations::{MemMigrations, Migration, MigrationsMut};
use propane::SqlType;
use propane_core::codegen::model_with_migrations;
use quote::quote;

#[test]
fn basic_current_migration() {
    let tokens = quote! {
        #[model]
        #[derive(PartialEq, Eq, Debug, Clone)]
        struct Foo {
            id: i64,
            bar: String,
        }
    };

    let mut ms = MemMigrations::new();
    model_with_migrations(tokens, &mut ms);
    let m = ms.current();
    let db = m.db().unwrap();
    assert_eq!(db.tables().count(), 1);
    let table = db.get_table("Foo").expect("No Foo table");
    let idcol = table.column("id").unwrap();
    assert_eq!(idcol.name(), "id");
    assert!(!idcol.nullable());
    assert!(idcol.is_pk());
    assert_eq!(*idcol.default(), None);
    assert_eq!(idcol.sqltype().unwrap(), SqlType::BigInt);
    assert!(!idcol.is_auto());

    let barcol = table.column("bar").unwrap();
    assert_eq!(barcol.name(), "bar");
    assert!(!barcol.nullable());
    assert!(!barcol.is_pk());
    assert_eq!(*barcol.default(), None);
    assert_eq!(barcol.sqltype().unwrap(), SqlType::Text);
    assert!(!barcol.is_auto());
}
