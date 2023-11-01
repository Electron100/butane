use butane_core::migrations::adb::*;
use butane_core::SqlType;

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

    let expected_op = Operation::RemoveTable("a".to_owned());
    assert_eq!(ops, vec![expected_op]);
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
