use super::*;
use propane_core::migrations;
use std::path::PathBuf;
use std::result::Result;
use syn::parse_quote;
use syn::{Field, ItemStruct, Type, TypePath};

pub fn write_table_to_disk(ast_struct: &ItemStruct) -> Result<(), Error> {
    let mut dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR expected to be set"),
    );
    dir.push("propane");
    dir.push("migrations");
    migrations::from_root(&dir)
        .get_migration("current")
        .write_table(&create_atable(ast_struct))
        .map_err(|e| e.into())
}

fn create_atable(ast_struct: &ItemStruct) -> ATable {
    let mut table = ATable::new(ast_struct.ident.to_string());
    let pk = pk_field(ast_struct).expect("No primary key found. Expected 'id' field or field with #[pk] attribute.");
    for f in ast_struct.fields.iter() {
        let name = f.ident.clone().expect("db object fields must be named").to_string();
        let col = AColumn::new(
            name,
            get_deferred_sql_type(&f),
            is_nullable(&f),
            f == &pk,
            get_default(&f),
        );
        table.add_column(col);
    }
    table
}

fn is_nullable(field: &Field) -> bool {
    let option: TypePath = parse_quote!(std::option::Option);
    match &field.ty {
        Type::Path(tp) => option == *tp,
        _ => false,
    }
}

fn get_default(field: &Field) -> Option<SqlVal> {
    // TODO support default values
    None
}
