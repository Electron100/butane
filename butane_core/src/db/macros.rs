#[macro_export]
macro_rules! connection_method_wrapper {
    ($ty:path) => {
        impl ConnectionMethods for $ty {
            fn execute(&self, sql: &str) -> Result<()> {
                ConnectionMethods::execute(self.wrapped_connection_methods()?, sql)
            }
            fn query<'a, 'b, 'c: 'a>(
                &'c self,
                table: &str,
                columns: &'b [Column],
                expr: Option<BoolExpr>,
                limit: Option<i32>,
                sort: Option<&[crate::query::Order]>,
            ) -> Result<RawQueryResult<'a>> {
                self.wrapped_connection_methods()?
                    .query(table, columns, expr, limit, sort)
            }
            fn insert_returning_pk(
                &self,
                table: &str,
                columns: &[Column],
                pkcol: &Column,
                values: &[SqlValRef<'_>],
            ) -> Result<SqlVal> {
                self.wrapped_connection_methods()?
                    .insert_returning_pk(table, columns, pkcol, values)
            }
            fn insert_only(
                &self,
                table: &str,
                columns: &[Column],
                values: &[SqlValRef<'_>],
            ) -> Result<()> {
                self.wrapped_connection_methods()?
                    .insert_only(table, columns, values)
            }
            fn insert_or_replace(
                &self,
                table: &str,
                columns: &[Column],
                pkcol: &Column,
                values: &[SqlValRef<'_>],
            ) -> Result<()> {
                self.wrapped_connection_methods()?
                    .insert_or_replace(table, columns, pkcol, values)
            }
            fn update(
                &self,
                table: &str,
                pkcol: Column,
                pk: SqlValRef,
                columns: &[Column],
                values: &[SqlValRef<'_>],
            ) -> Result<()> {
                self.wrapped_connection_methods()?
                    .update(table, pkcol, pk, columns, values)
            }
            fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
                self.wrapped_connection_methods()?.delete_where(table, expr)
            }
            fn has_table(&self, table: &str) -> Result<bool> {
                self.wrapped_connection_methods()?.has_table(table)
            }
        }
    };
}
