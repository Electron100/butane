use super::*;
use crate::migrations::adb::{AColumn, ATable, MANY_SUFFIX};
use crate::migrations::{MigrationMut, MigrationsMut};
use crate::Result;
use syn::{Field, ItemStruct};

pub fn write_table_to_disk<M>(
    ms: &mut impl MigrationsMut<M = M>,
    ast_struct: &ItemStruct,
    config: &dbobj::Config,
) -> Result<()>
where
    M: MigrationMut,
{
    let current_migration = ms.current();
    for table in create_atables(ast_struct, config) {
        current_migration.write_table(&table)?;
    }
    if let Some(name) = &config.table_name {
        // Custom table name, need to also be able to map with the type name
        current_migration.add_type(
            TypeKey::PK(ast_struct.ident.to_string()),
            DeferredSqlType::Deferred(TypeKey::PK(name.clone())),
        )?;
    }

    Ok(())
}

fn create_atables(ast_struct: &ItemStruct, config: &dbobj::Config) -> Vec<ATable> {
    let name = match &config.table_name {
        Some(n) => n.clone(),
        None => ast_struct.ident.to_string(),
    };
    let mut table = ATable::new(name);
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
                is_nullable(f),
                f == &pk,
                is_auto(f),
                is_unique(f),
                get_default(f).expect("Malformed default attribute"),
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
    let mut table = ATable::new(format!("{main_table_name}_{field_name}{MANY_SUFFIX}"));
    let col = AColumn::new_simple("owner", get_deferred_sql_type(&pk_field.ty));
    table.add_column(col);
    let col = AColumn::new_simple(
        "has",
        get_many_sql_type(many_field)
            .unwrap_or_else(|| panic!("Mis-identified Many field {field_name}")),
    );
    table.add_column(col);
    table
}

fn is_nullable(field: &Field) -> bool {
    is_option(field)
}
