#[macro_export]
macro_rules! connection_method_wrapper {
    ($ty:path) => {
        #[maybe_async_cfg::maybe(
            idents(
                Connection(sync = "Connection"),
                ConnectionMethods(sync = "ConnectionMethods"),
                Transaction(sync = "Transaction")
            ),
            sync(keep_self),
            async(feature = "async")
        )]
        #[async_trait::async_trait]
        impl ConnectionMethods for $ty {
            async fn execute(&self, sql: &str) -> Result<()> {
                ConnectionMethods::execute(self.wrapped_connection_methods()?, sql).await
            }
            async fn query<'c>(
                &'c self,
                table: &str,
                columns: &[Column],
                expr: Option<BoolExpr>,
                limit: Option<i32>,
                offset: Option<i32>,
                sort: Option<&[$crate::query::Order]>,
            ) -> Result<RawQueryResult<'c>> {
                self.wrapped_connection_methods()?
                    .query(table, columns, expr, limit, offset, sort)
                    .await
            }
            async fn insert_returning_pk(
                &self,
                table: &str,
                columns: &[Column],
                pkcol: &Column,
                values: &[SqlValRef<'_>],
            ) -> Result<SqlVal> {
                self.wrapped_connection_methods()?
                    .insert_returning_pk(table, columns, pkcol, values)
                    .await
            }
            async fn insert_only(
                &self,
                table: &str,
                columns: &[Column],
                values: &[SqlValRef<'_>],
            ) -> Result<()> {
                self.wrapped_connection_methods()?
                    .insert_only(table, columns, values)
                    .await
            }
            async fn insert_or_replace(
                &self,
                table: &str,
                columns: &[Column],
                pkcol: &Column,
                values: &[SqlValRef<'_>],
            ) -> Result<()> {
                self.wrapped_connection_methods()?
                    .insert_or_replace(table, columns, pkcol, values)
                    .await
            }
            async fn update(
                &self,
                table: &str,
                pkcol: Column,
                pk: SqlValRef<'_>,
                columns: &[Column],
                values: &[SqlValRef<'_>],
            ) -> Result<()> {
                self.wrapped_connection_methods()?
                    .update(table, pkcol, pk, columns, values)
                    .await
            }
            async fn delete(&self, table: &str, pkcol: &'static str, pk: SqlVal) -> Result<()> {
                self.wrapped_connection_methods()?
                    .delete(table, pkcol, pk)
                    .await
            }
            async fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
                self.wrapped_connection_methods()?
                    .delete_where(table, expr)
                    .await
            }
            async fn has_table(&self, table: &str) -> Result<bool> {
                self.wrapped_connection_methods()?.has_table(table).await
            }
        }
    };
}
