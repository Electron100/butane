use super::*;
use propane_core::migrations;
use std::path::PathBuf;
use std::result::Result;
use syn::parse_quote;
use syn::{Attribute, Field, ItemStruct, Type, TypePath};

pub fn write_table_to_disk(ast_struct: &ItemStruct) -> Result<(), Error> {
    let mut dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR expected to be set"),
    );
    dir.push("propane");
    dir.push("migrations");
    migrations::from_root(&dir)
        .get_migration("current")
        .write_table(&create_atable(ast_struct))
}

fn create_atable(ast_struct: &ItemStruct) -> ATable {
    let mut table = ATable::new(ast_struct.ident.to_string());
    for f in ast_struct.fields.iter() {
        let name = f.ident.clone().expect("db object fields must be named").to_string();
        let col = AColumn::new(
            name,
            get_deferred_sql_type(&f),
            is_nullable(&f),
            is_pk(&f),
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

fn is_pk(field: &Field) -> bool {
    has_attr(&field.attrs, "pk")
}

fn has_attr(attrs: &Vec<Attribute>, name: &str) -> bool {
    attrs
        .iter()
        .find(|a| match a.parse_meta() {
            Ok(m) => m.name().to_string() == name,
            _ => false,
        })
        .is_some()
}

fn get_default(field: &Field) -> Option<SqlVal> {
    // TODO support default values
    None
}
