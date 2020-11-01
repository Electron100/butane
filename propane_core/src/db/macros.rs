#[macro_export]
macro_rules! connection_method_wrapper {
    ($ty:path) => {
        impl ConnectionMethods for $ty {
            fn execute(&self, sql: &str) -> Result<()> {
                self.wrapped_connection_methods()?.execute(sql)
            }
            fn query(
                &self,
                table: &'static str,
                columns: &[Column],
                expr: Option<BoolExpr>,
                limit: Option<i32>,
            ) -> Result<RawQueryResult> {
                self.wrapped_connection_methods()?
                    .query(table, columns, expr, limit)
            }
            fn insert(
                &self,
                table: &'static str,
                columns: &[Column],
                pkcol: Column,
                values: &[SqlVal],
            ) -> Result<SqlVal> {
                self.wrapped_connection_methods()?
                    .insert(table, columns, pkcol, values)
            }
            fn insert_or_replace(
                &self,
                table: &'static str,
                columns: &[Column],
                values: &[SqlVal],
            ) -> Result<()> {
                self.wrapped_connection_methods()?
                    .insert_or_replace(table, columns, values)
            }
            fn update(
                &self,
                table: &'static str,
                pkcol: Column,
                pk: SqlVal,
                columns: &[Column],
                values: &[SqlVal],
            ) -> Result<()> {
                self.wrapped_connection_methods()?
                    .update(table, pkcol, pk, columns, values)
            }
            fn delete_where(&self, table: &'static str, expr: BoolExpr) -> Result<usize> {
                self.wrapped_connection_methods()?.delete_where(table, expr)
            }
            fn has_table(&self, table: &'static str) -> Result<bool> {
                self.wrapped_connection_methods()?.has_table(table)
            }
        }
    };
}
