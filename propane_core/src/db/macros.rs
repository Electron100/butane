#[macro_export]
macro_rules! connection_method_wrapper {
    ($ty:path) => {
        impl ConnectionMethods for $ty {
            fn backend_name(&self) -> &'static str {
                self.conn.backend_name()
            }
            fn execute(&self, sql: &str) -> Result<()> {
                self.conn.execute(sql)
            }
            fn query(
                &self,
                table: &'static str,
                columns: &[Column],
                expr: Option<BoolExpr>,
                limit: Option<i32>,
            ) -> Result<RawQueryResult> {
                self.conn.query(table, columns, expr, limit)
            }
            fn insert_or_replace(
                &self,
                table: &'static str,
                columns: &[Column],
                values: &[SqlVal],
            ) -> Result<()> {
                self.conn.insert_or_replace(table, columns, values)
            }
            fn delete(&self, table: &'static str, pkcol: &'static str, pk: &SqlVal) -> Result<()> {
                self.conn.delete(table, pkcol, pk)
            }
            fn has_table(&self, table: &'static str) -> Result<bool> {
                self.conn.has_table(table)
            }
        }
    };
}
