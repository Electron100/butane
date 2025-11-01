//! Tests for ArrayString support in butane_core

use arrayvec::ArrayString;
use butane_core::{Error::CannotConvertSqlVal, FromSql, SqlType, SqlVal, SqlValRef, ToSql};

#[test]
fn array_string_to_sql() {
    let mut array_str = ArrayString::<32>::new();
    array_str.push_str("hello world");

    let sql_val = array_str.to_sql();
    assert_eq!(sql_val, SqlVal::Text("hello world".to_string()));
}

#[test]
fn array_string_to_sql_ref() {
    let mut array_str = ArrayString::<16>::new();
    array_str.push_str("test");

    let sql_val_ref = array_str.to_sql_ref();
    if let SqlValRef::Text(text) = sql_val_ref {
        assert_eq!(text, "test");
    } else {
        panic!("Expected SqlValRef::Text");
    }
}

#[test]
fn array_string_into_sql() {
    let mut array_str = ArrayString::<64>::new();
    array_str.push_str("into test");

    let sql_val = array_str.into_sql();
    assert_eq!(sql_val, SqlVal::Text("into test".to_string()));
}

#[test]
fn array_string_from_sql_ref() {
    let sql_val_ref = SqlValRef::Text("from sql ref");
    let array_str: ArrayString<32> = ArrayString::from_sql_ref(sql_val_ref).unwrap();
    assert_eq!(array_str.as_str(), "from sql ref");
}

#[test]
fn array_string_from_sql() {
    let sql_val = SqlVal::Text("from sql".to_string());
    let array_str: ArrayString<16> = ArrayString::from_sql(sql_val).unwrap();
    assert_eq!(array_str.as_str(), "from sql");
}

#[test]
fn array_string_from_sql_too_long() {
    let long_text = "this string is definitely longer than 8 characters";
    let sql_val = SqlVal::Text(long_text.to_string());

    // Should fail when the string is too long for the ArrayString capacity
    let result: Result<ArrayString<8>, _> = ArrayString::from_sql(sql_val);
    assert!(result.is_err());

    if let Err(CannotConvertSqlVal(SqlType::Text, _)) = result {
        // Expected error type
    } else {
        panic!("Expected CannotConvertSqlVal error");
    }
}

#[test]
fn array_string_from_sql_ref_too_long() {
    let long_text = "this string is too long for a 4 character array";
    let sql_val_ref = SqlValRef::Text(long_text);

    // Should fail when the string is too long for the ArrayString capacity
    let result: Result<ArrayString<4>, _> = ArrayString::from_sql_ref(sql_val_ref);
    assert!(result.is_err());

    if let Err(CannotConvertSqlVal(SqlType::Text, _)) = result {
        // Expected error type
    } else {
        panic!("Expected CannotConvertSqlVal error");
    }
}

#[test]
fn array_string_from_wrong_sql_type() {
    let sql_val = SqlVal::Int(42);
    let result: Result<ArrayString<16>, _> = ArrayString::from_sql(sql_val);
    assert!(result.is_err());

    let sql_val_ref = SqlValRef::Bool(true);
    let result: Result<ArrayString<16>, _> = ArrayString::from_sql_ref(sql_val_ref);
    assert!(result.is_err());
}

#[test]
fn array_string_field_type() {
    use butane_core::FieldType;

    // Test that ArrayString implements FieldType correctly
    assert_eq!(<ArrayString<32> as FieldType>::SQLTYPE, SqlType::Text);
    assert_eq!(<ArrayString<255> as FieldType>::SQLTYPE, SqlType::Text);
}

#[test]
fn array_string_primary_key_type() {
    use butane_core::PrimaryKeyType;

    let mut pk1 = ArrayString::<16>::new();
    pk1.push_str("pk1");

    let mut pk2 = ArrayString::<16>::new();
    pk2.push_str("pk2");

    // Test that ArrayString can be used as a primary key
    assert!(pk1.is_valid());
    assert!(pk2.is_valid());
    assert_ne!(pk1, pk2);

    // Test cloning (required for PrimaryKeyType)
    let pk1_clone = pk1.clone();
    assert_eq!(pk1, pk1_clone);
}

#[test]
fn array_string_serialization() {
    let mut array_str = ArrayString::<32>::new();
    array_str.push_str("serialize test");

    let sql_val = array_str.to_sql();
    let serialized = serde_json::to_string(&sql_val).unwrap();
    assert_eq!(serialized, "{\"Text\":\"serialize test\"}");

    // Test deserialization
    let deserialized: SqlVal = serde_json::from_str(&serialized).unwrap();
    if let SqlVal::Text(text) = deserialized {
        assert_eq!(text, "serialize test");
    } else {
        panic!("Expected SqlVal::Text");
    }
}

#[test]
fn array_string_empty() {
    let empty_str = ArrayString::<10>::new();

    let sql_val = empty_str.to_sql();
    assert_eq!(sql_val, SqlVal::Text("".to_string()));

    let sql_val_ref = empty_str.to_sql_ref();
    if let SqlValRef::Text(text) = sql_val_ref {
        assert_eq!(text, "");
    } else {
        panic!("Expected SqlValRef::Text");
    }
}

#[test]
fn array_string_max_capacity() {
    let mut array_str = ArrayString::<5>::new();
    array_str.push_str("12345"); // Exactly at capacity

    let sql_val = array_str.to_sql();
    assert_eq!(sql_val, SqlVal::Text("12345".to_string()));

    // Test round-trip
    let recovered: ArrayString<5> = ArrayString::from_sql(sql_val).unwrap();
    assert_eq!(recovered.as_str(), "12345");
}
