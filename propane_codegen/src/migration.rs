use super::*;
use propane_core::migrations;
use propane_core::migrations::adb::{AColumn, ATable};
use propane_core::Result;
use std::path::PathBuf;
use syn::{Field, ItemStruct};

pub fn write_table_to_disk(ast_struct: &ItemStruct) -> Result<()> {
    let dir = migrations_dir();
    let current_migration = migrations::from_root(&dir).current();
    for table in create_atables(ast_struct) {
        current_migration.write_table(&table)?;
    }
    Ok(())
}

pub fn add_typedef(alias: &syn::Ident, orig: &syn::Type) -> Result<()> {
    let current_migration = migrations::from_root(&migrations_dir()).current();
    let key = TypeKey::CustomType(alias.to_string());
    current_migration.add_type(key, get_deferred_sql_type(orig))
}

fn migrations_dir() -> PathBuf {
    let mut dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR expected to be set"),
    );
    dir.push("propane");
    dir.push("migrations");
    dir
}

fn create_atables(ast_struct: &ItemStruct) -> Vec<ATable> {
    let mut table = ATable::new(ast_struct.ident.to_string());
    let pk = pk_field(ast_struct)
        .expect("No primary key found. Expected 'id' field or field with #[pk] attribute.");
    let mut result: Vec<ATable> = Vec::new();
    for f in fields(ast_struct) {
        let name = f
            .ident
            .clone()
            .expect("db object fields must be named")
            .to_string();
        if is_row_field(f) {
            let col = AColumn::new(
                name,
                get_deferred_sql_type(&f.ty),
                is_nullable(&f),
                f == &pk,
                is_auto(&f),
            );
            table.add_column(col);
        } else if is_many_to_many(f) {
            result.push(many_table(&table.name, f, &pk));
        }
    }
    result.push(table);
    result
}

fn many_table(main_table_name: &str, many_field: &Field, pk_field: &Field) -> ATable {
    let field_name = many_field
        .ident
        .clone()
        .expect("fields must be named")
        .to_string();
    let mut table = ATable::new(format!("{}_{}_Many", main_table_name, field_name));
    let col = AColumn::new(
        "owner",
        get_deferred_sql_type(&pk_field.ty),
        false,
        false,
        false,
    );
    table.add_column(col);
    let col = AColumn::new(
        "has",
        get_many_sql_type(many_field).expect(&format!("Mis-identified Many field {}", field_name)),
        false,
        false,
        false,
    );
    table.add_column(col);
    table
}

fn is_nullable(field: &Field) -> bool {
    is_option(field)
}
