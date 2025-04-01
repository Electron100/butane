use butane_core::db::ConnectionAsync;
use butane_core::migrations::adb::*;
use butane_core::SqlType;
use butane_test_helper::*;
use butane_test_macros::butane_test;

#[test]
fn empty_diff() {
    let old = ADB::default();
    let new = ADB::default();
    let ops = diff(&old, &new);
    assert_eq!(ops, vec![]);
}

#[test]
fn add_table() {
    let old = ADB::default();
    let mut new = ADB::default();
    let mut table = ATable::new("a".to_owned());
    let column = AColumn::new_simple(
        "a".to_owned(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
    );
    table.add_column(column);
    new.replace_table(table.clone());

    let ops = diff(&old, &new);

    assert_eq!(ops, vec![Operation::AddTable(table)]);
}

#[test]
fn remove_table() {
    let mut old = ADB::default();
    let new = ADB::default();
    let mut table = ATable::new("a".to_owned());
    let column = AColumn::new_simple(
        "a".to_owned(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
    );
    table.add_column(column);
    old.replace_table(table.clone());

    let ops = diff(&old, &new);

    let expected_ops = vec![
        Operation::RemoveTableConstraints(table),
        Operation::RemoveTable("a".to_owned()),
    ];
    assert_eq!(ops, expected_ops);
}

#[test]
fn stable_table_alpha_order() {
    let old = ADB::default();
    let mut new = ADB::default();

    // Insert tables out of order
    let mut table_b = ATable::new("b".to_owned());
    let column_b = AColumn::new_simple(
        "b".to_owned(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
    );
    table_b.add_column(column_b);
    new.replace_table(table_b.clone());

    let mut table_a = ATable::new("a".to_owned());
    let column_a = AColumn::new_simple(
        "a".to_owned(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
    );
    table_a.add_column(column_a);
    new.replace_table(table_a.clone());

    let ops = diff(&old, &new);

    assert_eq!(
        ops,
        vec![Operation::AddTable(table_a), Operation::AddTable(table_b)]
    );
}

#[test]
fn stable_new_table_column_insert_order() {
    let old = ADB::default();
    let mut new = ADB::default();
    let mut table = ATable::new("a".to_owned());

    // Insert columns out of order
    let column_b = AColumn::new_simple(
        "b".to_owned(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
    );
    table.add_column(column_b.clone());

    let column_a = AColumn::new_simple(
        "a".to_owned(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
    );
    table.add_column(column_a.clone());

    // Add updated table to new
    new.replace_table(table.clone());

    let ops = diff(&old, &new);

    // Columns remain in insertion order
    assert_eq!(ops, vec![Operation::AddTable(table)]);
}

#[test]
fn stable_add_column_alpha_order() {
    let mut old = ADB::default();
    let mut new = ADB::default();
    let mut table = ATable::new("a".to_owned());

    // Add empty table to old
    old.replace_table(table.clone());

    // Insert columns out of order
    let column_b = AColumn::new_simple(
        "b".to_owned(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
    );
    table.add_column(column_b.clone());

    let column_a = AColumn::new_simple(
        "a".to_owned(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
    );
    table.add_column(column_a.clone());

    // Add updated table to new
    new.replace_table(table.clone());

    let ops = diff(&old, &new);

    assert_eq!(
        ops,
        vec![
            Operation::AddColumn("a".to_owned(), column_a),
            Operation::AddColumn("a".to_owned(), column_b),
        ]
    );
}

#[test]
fn add_table_fkey() {
    let known_int_type = DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Int));

    let old = ADB::default();
    let mut new = ADB::default();
    let mut table_a = ATable::new("a".to_owned());
    let column = AColumn::new(
        "id".to_owned(),
        known_int_type.clone(),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        None,  // reference
    );
    table_a.add_column(column);
    new.replace_table(table_a.clone());

    let mut table_b = ATable::new("b".to_owned());
    let column = AColumn::new(
        "fkey".to_owned(),
        DeferredSqlType::Deferred(TypeKey::PK("a".to_owned())),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Deferred(DeferredSqlType::Deferred(TypeKey::PK(
            "a".to_owned(),
        )))),
    );
    table_b.add_column(column);
    new.replace_table(table_b.clone());

    new.resolve_types().unwrap();

    let mut resolved_table_b = ATable::new("b".to_owned());
    let column = AColumn::new(
        "fkey".to_owned(),
        known_int_type.clone(),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Literal(ARefLiteral::new(
            "a".to_owned(),
            "id".to_owned(),
        ))),
    );
    resolved_table_b.add_column(column);

    let ops = diff(&old, &new);

    assert_eq!(
        ops,
        vec![
            Operation::AddTable(table_a),
            Operation::AddTable(resolved_table_b.clone()),
            Operation::AddTableConstraints(resolved_table_b),
        ]
    );
}

/// This is the same as test "add_table_fkey", except that it
/// runs the DDL on a database, and then deletes the column.
#[butane_test(nomigrate)]
async fn add_table_fkey_delete_column(conn: ConnectionAsync) {
    let known_int_type = DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Int));

    let old = ADB::default();
    let mut new = ADB::default();
    let mut table_a = ATable::new("a".to_owned());
    let id_column = AColumn::new(
        "id".to_owned(),
        known_int_type.clone(),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        None,  // reference
    );
    table_a.add_column(id_column.clone());
    new.replace_table(table_a.clone());

    let mut table_b = ATable::new("b".to_owned());
    table_b.add_column(id_column.clone());
    let column = AColumn::new(
        "fkey".to_owned(),
        DeferredSqlType::Deferred(TypeKey::PK("a".to_owned())),
        false, // nullable
        false, // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Deferred(DeferredSqlType::Deferred(TypeKey::PK(
            "a".to_owned(),
        )))),
    );
    table_b.add_column(column);
    new.replace_table(table_b.clone());

    new.resolve_types().unwrap();

    let mut resolved_table_b = ATable::new("b".to_owned());
    resolved_table_b.add_column(id_column);
    let column = AColumn::new(
        "fkey".to_owned(),
        known_int_type.clone(),
        false, // nullable
        false, // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Literal(ARefLiteral::new(
            "a".to_owned(),
            "id".to_owned(),
        ))),
    );
    resolved_table_b.add_column(column);

    let ops = diff(&old, &new);

    assert_eq!(
        ops,
        vec![
            Operation::AddTable(table_a),
            Operation::AddTable(resolved_table_b.clone()),
            Operation::AddTableConstraints(resolved_table_b),
        ]
    );

    let backend = conn.backend();
    let sql = backend.create_migration_sql(&new, ops).unwrap();

    conn.execute(&sql).await.unwrap();
    conn.execute("SELECT * from a").await.unwrap();
    conn.execute("SELECT * from b").await.unwrap();

    // "ALTER TABLE b DROP COLUMN fkey;" fails due to sqlite not being
    // able to remove the attached constraint, however the RemoveColumn
    // operation already recreates the table, so this works.
    let remove_column_op = Operation::RemoveColumn("b".to_owned(), "fkey".to_owned());
    let sql = backend
        .create_migration_sql(&new, vec![remove_column_op])
        .unwrap();
    conn.execute(&sql).await.unwrap();
}

/// This is the same as test "add_table_fkey", except that it
/// intentionally links a column on table a to table b, and
/// it runs the DDL on a database.
#[butane_test(nomigrate)]
async fn add_table_fkey_back_reference(conn: ConnectionAsync) {
    let known_int_type = DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Int));

    let old = ADB::default();
    let mut new = ADB::default();
    let mut table_b = ATable::new("b".to_owned());
    let column = AColumn::new(
        "id".to_owned(),
        known_int_type.clone(),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        None,  // reference
    );
    table_b.add_column(column);
    new.replace_table(table_b.clone());

    let mut table_a = ATable::new("a".to_owned());
    let column = AColumn::new(
        "fkey".to_owned(),
        DeferredSqlType::Deferred(TypeKey::PK("b".to_owned())),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Deferred(DeferredSqlType::Deferred(TypeKey::PK(
            "b".to_owned(),
        )))),
    );
    table_a.add_column(column);
    new.replace_table(table_a.clone());

    new.resolve_types().unwrap();

    let mut resolved_table_a = ATable::new("a".to_owned());
    let column = AColumn::new(
        "fkey".to_owned(),
        known_int_type.clone(),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Literal(ARefLiteral::new(
            "b".to_owned(),
            "id".to_owned(),
        ))),
    );
    resolved_table_a.add_column(column);

    let ops = diff(&old, &new);

    assert_eq!(
        ops,
        vec![
            Operation::AddTable(resolved_table_a.clone()),
            Operation::AddTable(table_b),
            Operation::AddTableConstraints(resolved_table_a),
        ]
    );

    let backend = conn.backend();
    let sql = backend.create_migration_sql(&new, ops).unwrap();
    let sql_lines: Vec<&str> = sql.lines().collect();
    if backend.name() == "sqlite" {
        assert_eq!(
            sql_lines,
            vec![
                "CREATE TABLE a (",
                "fkey INTEGER NOT NULL PRIMARY KEY,",
                "FOREIGN KEY (fkey) REFERENCES b(\"id\")",
                ");",
                "CREATE TABLE b (",
                "\"id\" INTEGER NOT NULL PRIMARY KEY",
                ");",
            ]
        );
    }

    conn.execute(&sql).await.unwrap();
    conn.execute("SELECT * from a").await.unwrap();
    conn.execute("SELECT * from b").await.unwrap();
}

/// This is the same as test "add_table_fkey", except that it
/// creates a table with multiple fkey constraints.
#[butane_test(nomigrate)]
async fn add_table_fkey_multiple(conn: ConnectionAsync) {
    let known_int_type = DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Int));

    let old = ADB::default();
    let mut new = ADB::default();

    let id_column = AColumn::new(
        "id".to_owned(),
        known_int_type.clone(),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        None,  // reference
    );

    let mut table_a = ATable::new("a".to_owned());
    table_a.add_column(id_column.clone());
    new.replace_table(table_a.clone());

    let mut table_b = ATable::new("b".to_owned());
    table_b.add_column(id_column.clone());
    new.replace_table(table_b.clone());

    let mut table_c = ATable::new("c".to_owned());
    table_c.add_column(id_column.clone());
    let column = AColumn::new(
        "fkey_a".to_owned(),
        DeferredSqlType::Deferred(TypeKey::PK("a".to_owned())),
        false, // nullable
        false, // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Deferred(DeferredSqlType::Deferred(TypeKey::PK(
            "a".to_owned(),
        )))),
    );
    table_c.add_column(column);
    let column = AColumn::new(
        "fkey_b".to_owned(),
        DeferredSqlType::Deferred(TypeKey::PK("b".to_owned())),
        false, // nullable
        false, // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Deferred(DeferredSqlType::Deferred(TypeKey::PK(
            "b".to_owned(),
        )))),
    );
    table_c.add_column(column);
    new.replace_table(table_c.clone());

    new.resolve_types().unwrap();

    let mut resolved_table_c = ATable::new("c".to_owned());
    resolved_table_c.add_column(id_column);
    let column = AColumn::new(
        "fkey_a".to_owned(),
        known_int_type.clone(),
        false, // nullable
        false, // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Literal(ARefLiteral::new(
            "a".to_owned(),
            "id".to_owned(),
        ))),
    );
    resolved_table_c.add_column(column);
    let column = AColumn::new(
        "fkey_b".to_owned(),
        known_int_type.clone(),
        false, // nullable
        false, // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Literal(ARefLiteral::new(
            "b".to_owned(),
            "id".to_owned(),
        ))),
    );
    resolved_table_c.add_column(column);

    let ops = diff(&old, &new);

    assert_eq!(
        ops,
        vec![
            Operation::AddTable(table_a),
            Operation::AddTable(table_b),
            Operation::AddTable(resolved_table_c.clone()),
            Operation::AddTableConstraints(resolved_table_c),
        ]
    );

    let backend = conn.backend();
    let sql = backend.create_migration_sql(&new, ops).unwrap();

    conn.execute(&sql).await.unwrap();
    conn.execute("SELECT * from a").await.unwrap();
    conn.execute("SELECT * from b").await.unwrap();
}

/// Creates the test case for adding a foreign key, returning the migration operations,
/// the target ADB, and the tables which should be expected to be created.
fn create_add_renamed_table_fkey_ops() -> (Vec<Operation>, ADB, ATable, ATable) {
    let known_int_type = DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Int));

    let old = ADB::default();
    let mut new = ADB::default();
    let mut table_a = ATable::new("a_table".to_owned());
    let column = AColumn::new(
        "id".to_owned(),
        known_int_type.clone(),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        None,  // reference
    );
    table_a.add_column(column);
    new.replace_table(table_a.clone());

    // Add the type "a" renamed to table "a_table"
    let type_name = TypeKey::PK("a".to_owned());
    let table_name = DeferredSqlType::Deferred(TypeKey::PK("a_table".to_owned()));
    new.add_type(type_name, table_name);

    let mut table_b = ATable::new("b".to_owned());
    let column = AColumn::new(
        "b".to_owned(),
        DeferredSqlType::Deferred(TypeKey::PK("a".to_owned())),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Deferred(DeferredSqlType::Deferred(TypeKey::PK(
            "a".to_owned(),
        )))),
    );
    table_b.add_column(column);
    new.replace_table(table_b.clone());

    new.resolve_types().unwrap();

    let mut resolved_table_b = ATable::new("b".to_owned());
    let column = AColumn::new(
        "b".to_owned(),
        known_int_type.clone(),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Literal(ARefLiteral::new(
            "a_table".to_owned(),
            "id".to_owned(),
        ))),
    );
    resolved_table_b.add_column(column);

    let ops = diff(&old, &new);

    (ops, new, table_a, resolved_table_b)
}

#[test]
fn add_renamed_table_fkey() {
    let (ops, _, table_a, resolved_table_b) = create_add_renamed_table_fkey_ops();

    assert_eq!(
        ops,
        vec![
            Operation::AddTable(table_a),
            Operation::AddTable(resolved_table_b.clone()),
            Operation::AddTableConstraints(resolved_table_b),
        ]
    );
}

#[test]
fn add_renamed_table_fkey_ddl_sqlite() {
    let (ops, new, ..) = create_add_renamed_table_fkey_ops();

    let backend = butane_core::db::get_backend("sqlite").unwrap();
    let sql = backend.create_migration_sql(&new, ops).unwrap();
    let sql_lines: Vec<&str> = sql.lines().collect();
    assert_eq!(
        sql_lines,
        vec![
            "CREATE TABLE a_table (",
            "\"id\" INTEGER NOT NULL PRIMARY KEY",
            ");",
            "CREATE TABLE b (",
            "b INTEGER NOT NULL PRIMARY KEY,",
            "FOREIGN KEY (b) REFERENCES a_table(\"id\")",
            ");",
        ]
    );
}

#[test]
fn add_renamed_table_fkey_ddl_pg() {
    let (ops, new, ..) = create_add_renamed_table_fkey_ops();

    let backend = butane_core::db::get_backend("pg").unwrap();
    let sql = backend.create_migration_sql(&new, ops).unwrap();
    let sql_lines: Vec<&str> = sql.lines().collect();
    assert_eq!(
        sql_lines,
        vec![
            "CREATE TABLE a_table (",
            "\"id\" INTEGER NOT NULL PRIMARY KEY",
            ");",
            "CREATE TABLE b (",
            "b INTEGER NOT NULL PRIMARY KEY",
            ");",
            "ALTER TABLE b ADD FOREIGN KEY (b) REFERENCES a_table(\"id\");",
        ]
    );
}

/// Creates the test case for adding a many table, returning the migration operations,
/// the target ADB, and the tables which should be expected to be created.
fn create_add_table_many_ops() -> (Vec<Operation>, ADB, ATable, ATable, ATable) {
    let known_int_type = DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Int));

    let old = ADB::default();
    let mut new = ADB::default();

    let id_column = AColumn::new(
        "id".to_owned(),
        known_int_type.clone(),
        false, // nullable
        true,  // pk
        false, // auto
        false, // unique
        None,  // default
        None,  // reference
    );

    let mut table_a = ATable::new("a".to_owned());
    table_a.add_column(id_column.clone());
    new.replace_table(table_a.clone());

    let mut table_b = ATable::new("b".to_owned());
    table_b.add_column(id_column);

    new.replace_table(table_b.clone());

    let many_table = butane_core::migrations::adb::create_many_table(
        "b",
        "many_a",
        DeferredSqlType::Deferred(TypeKey::PK("a".to_owned())),
        "id",
        known_int_type.clone(),
    );
    new.replace_table(many_table.clone());

    new.resolve_types().unwrap();

    let mut resolved_many_table = ATable::new(many_table.name);
    let resolved_owner_column = AColumn::new(
        "owner",
        known_int_type.clone(),
        false, // nullable
        false, // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Literal(ARefLiteral::new(
            "b".to_owned(),
            "id".to_owned(),
        ))),
    );
    let resolved_has_column = AColumn::new(
        "has",
        known_int_type.clone(),
        false, // nullable
        false, // pk
        false, // auto
        false, // unique
        None,  // default
        Some(ARef::Literal(ARefLiteral::new(
            "a".to_owned(),
            "id".to_owned(),
        ))),
    );
    resolved_many_table.add_column(resolved_owner_column);
    resolved_many_table.add_column(resolved_has_column);

    let ops = diff(&old, &new);
    (ops, new, table_a, table_b, resolved_many_table)
}

#[test]
fn add_table_many() {
    let (ops, _, table_a, table_b, resolved_many_table) = create_add_table_many_ops();

    assert_eq!(ops[0], Operation::AddTable(table_a.clone()));
    assert_eq!(ops[1], Operation::AddTable(table_b.clone()));
    assert_eq!(ops[2], Operation::AddTable(resolved_many_table.clone()));

    assert_eq!(
        ops,
        vec![
            Operation::AddTable(table_a),
            Operation::AddTable(table_b.clone()),
            Operation::AddTable(resolved_many_table.clone()),
            Operation::AddTableConstraints(resolved_many_table.clone()),
        ]
    );
}

#[test]
fn add_table_many_ddl_sqlite() {
    let (ops, new, ..) = create_add_table_many_ops();

    let backend = butane_core::db::get_backend("sqlite").unwrap();
    let sql = backend.create_migration_sql(&new, ops).unwrap();
    let sql_lines: Vec<&str> = sql.lines().collect();
    assert_eq!(
        sql_lines,
        vec![
            "CREATE TABLE a (",
            "\"id\" INTEGER NOT NULL PRIMARY KEY",
            ");",
            "CREATE TABLE b (",
            "\"id\" INTEGER NOT NULL PRIMARY KEY",
            ");",
            "CREATE TABLE b_many_a_Many (",
            "\"owner\" INTEGER NOT NULL,",
            "has INTEGER NOT NULL,",
            "FOREIGN KEY (\"owner\") REFERENCES b(\"id\")",
            "FOREIGN KEY (has) REFERENCES a(\"id\")",
            ");",
        ]
    );
}

#[test]
fn add_table_many_ddl_pg() {
    let (ops, new, ..) = create_add_table_many_ops();

    let backend = butane_core::db::get_backend("pg").unwrap();
    let sql = backend.create_migration_sql(&new, ops).unwrap();
    let sql_lines: Vec<&str> = sql.lines().collect();
    assert_eq!(
        sql_lines,
        vec![
            "CREATE TABLE a (",
            "\"id\" INTEGER NOT NULL PRIMARY KEY",
            ");",
            "CREATE TABLE b (",
            "\"id\" INTEGER NOT NULL PRIMARY KEY",
            ");",
            "CREATE TABLE b_many_a_Many (",
            "\"owner\" INTEGER NOT NULL,",
            "has INTEGER NOT NULL",
            ");",
            "ALTER TABLE b_many_a_Many ADD FOREIGN KEY (\"owner\") REFERENCES b(\"id\");",
            "ALTER TABLE b_many_a_Many ADD FOREIGN KEY (has) REFERENCES a(\"id\");",
        ]
    );
}
