#[cfg(feature = "uuid")]
mod tests {
    use butane_core::{Error::CannotConvertSqlVal, FromSql, SqlType, ToSql};

    #[test]
    fn sqlval_blob_serde() {
        let input = uuid::uuid!("97f40d6c-e39b-47e5-b145-1edbd599861f");
        let sql_val = input.to_sql();
        let s = serde_json::to_string(&sql_val).unwrap();
        assert_eq!(
            s,
            "{\"Blob\":[151,244,13,108,227,155,71,229,177,69,30,219,213,153,134,31]}"
        );
    }

    #[test]
    fn sqlval_text_serde() {
        let sql_val = butane_core::SqlVal::Text("97f40d6c-e39b-47e5-b145-1edbd599861f".to_string());
        let s = serde_json::to_string(&sql_val).unwrap();
        assert_eq!(s, "{\"Text\":\"97f40d6c-e39b-47e5-b145-1edbd599861f\"}");
    }

    #[test]
    fn sqlval_other_causes_error() {
        let sql_val = butane_core::SqlVal::Null;
        let sql_val_ref = sql_val.as_ref();
        let rv = uuid::Uuid::from_sql_ref(sql_val_ref).unwrap_err();
        assert_matches::assert_matches!(rv, CannotConvertSqlVal(SqlType::Blob, _));
    }

    #[test]
    fn sqlval_text_from_sql() {
        let sql_val = butane_core::SqlVal::Text("97f40d6c-e39b-47e5-b145-1edbd599861f".to_string());
        let sql_val_ref = sql_val.as_ref();
        let rv = uuid::Uuid::from_sql_ref(sql_val_ref).unwrap();
        assert_eq!(rv, uuid::uuid!("97f40d6c-e39b-47e5-b145-1edbd599861f"));
    }
}
