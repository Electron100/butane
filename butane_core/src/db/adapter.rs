//! Adapter between sync and async connections
//! Allows implementing an async trait in terms of synchronous
//! operations without blocking the task. It accomplishes this by
//! running the blocking operations on a dedicated thread and communicating
//! between threads.

use super::*;
use crate::query::Order;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use tokio::sync::oneshot;

enum Command {
    Func(Box<dyn FnOnce() + Send>),
    Shutdown,
}

#[derive(Debug)]
struct AsyncAdapterEnv {
    sender: crossbeam_channel::Sender<Command>,
    thread_handle: Option<JoinHandle<()>>,
}

impl AsyncAdapterEnv {
    fn new() -> Self {
        // We spawn off a new thread and do all the blocking/sqlite work on that thread. Much of this is inspired
        // by the crate async_sqlite
        let (sender, receiver) = crossbeam_channel::unbounded();
        let thread_handle = thread::spawn(move || {
            while let Ok(cmd) = receiver.recv() {
                match cmd {
                    Command::Func(func) => func(),
                    Command::Shutdown => {
                        // TODO should connection support an explicit close?
                        return;
                    }
                }
            }
        });
        Self {
            sender,
            thread_handle: Some(thread_handle),
        }
    }

    async fn invoke<'c, 's, 'result, F, T, U>(
        &'s self,
        context: &SyncSendPtrMut<T>,
        func: F,
    ) -> Result<U>
    // todo can this just be result
    where
        F: FnOnce(&'c T) -> Result<U> + Send,
        F: 'result,
        U: Send + 'result,
        T: ?Sized + 'c, // TODO should this be Send
        's: 'result,
        'c: 'result,
    {
        // todo parts of this can be shared with the other two invoke functions
        let (tx, rx) = oneshot::channel();
        let context_ptr = SendPtr::new(context.inner);
        let func_taking_ptr = |ctx: SendPtr<T>| func(unsafe { ctx.inner.as_ref() }.unwrap());
        let wrapped_func = move || _ = tx.send(func_taking_ptr(context_ptr));
        let boxed_func: Box<dyn FnOnce() + Send + 'result> = Box::new(wrapped_func);
        let static_func: Box<dyn FnOnce() + Send + 'static> =
            unsafe { std::mem::transmute(boxed_func) };
        self.sender.send(Command::Func(static_func))?;
        // https://stackoverflow.com/questions/52424449/is-there-a-way-to-express-same-generic-type-with-different-lifetime-bound
        //https://docs.rs/crossbeam/0.8.2/crossbeam/fn.scope.html
        // TODO ensure soundness and document why
        rx.await?
    }

    async fn invoke_mut<'c, 's, 'result, F, T, U>(
        &'s self,
        context: &SyncSendPtrMut<T>,
        func: F,
    ) -> Result<U>
    where
        F: FnOnce(&'c mut T) -> Result<U> + Send,
        F: 'result,
        U: Send + 'result,
        T: ?Sized + 'c, // TODO should this be Send
        's: 'result,
        'c: 'result,
    {
        let (tx, rx) = oneshot::channel();
        let context_ptr = SendPtrMut::new(context.inner);
        let func_taking_ptr = |ctx: SendPtrMut<T>| func(unsafe { ctx.inner.as_mut().unwrap() });
        let wrapped_func = move || _ = tx.send(func_taking_ptr(context_ptr));
        let boxed_func: Box<dyn FnOnce() + Send + 'result> = Box::new(wrapped_func);
        let static_func: Box<dyn FnOnce() + Send + 'static> =
            unsafe { std::mem::transmute(boxed_func) };
        self.sender.send(Command::Func(static_func))?;
        // https://stackoverflow.com/questions/52424449/is-there-a-way-to-express-same-generic-type-with-different-lifetime-bound
        //https://docs.rs/crossbeam/0.8.2/crossbeam/fn.scope.html
        // TODO ensure soundness and document why
        rx.await?
    }

    fn invoke_blocking<'c, 's, 'result, F, T, U>(&'s self, context: *const T, func: F) -> Result<U>
    where
        F: FnOnce(&'c T) -> Result<U> + Send,
        F: 'result,
        U: Send + 'result,
        T: ?Sized + 'c,
        's: 'result,
        'c: 'result,
    {
        let (tx, rx) = crossbeam_channel::unbounded();
        let context_ptr = SendPtr::new(context);
        let func_taking_ptr = |ctx: SendPtr<T>| func(unsafe { ctx.inner.as_ref() }.unwrap());
        let wrapped_func = move || _ = tx.send(func_taking_ptr(context_ptr));
        let boxed_func: Box<dyn FnOnce() + Send + 'result> = Box::new(wrapped_func);
        let static_func: Box<dyn FnOnce() + Send + 'static> =
            unsafe { std::mem::transmute(boxed_func) };
        self.sender.send(Command::Func(static_func))?;
        // TODO ensure soundness and document why
        rx.recv()?
    }
}

impl Drop for AsyncAdapterEnv {
    fn drop(&mut self) {
        self.sender
            .send(Command::Shutdown)
            .expect("Cannot send async adapter env shutdown command, cannot join thread");
        self.thread_handle.take().map(|h| h.join());
    }
}

struct SendPtr<T: ?Sized> {
    inner: *const T,
}
impl<T: ?Sized> SendPtr<T> {
    fn new(inner: *const T) -> Self {
        Self { inner }
    }
}
unsafe impl<T: ?Sized> Send for SendPtr<T> {}

struct SendPtrMut<T: ?Sized> {
    inner: *mut T,
}
impl<T: ?Sized> SendPtrMut<T> {
    fn new(inner: *mut T) -> Self {
        Self { inner }
    }
}
unsafe impl<T: ?Sized> Send for SendPtrMut<T> {}

struct SyncSendPtrMut<T: ?Sized> {
    inner: *mut T,
}
impl<T: ?Sized> SyncSendPtrMut<T> {
    fn new(inner: *mut T) -> Self {
        // todo should this be unsafe
        Self { inner }
    }
}
impl<T> From<T> for SyncSendPtrMut<T>
where
    T: Debug + Sized,
{
    fn from(val: T) -> Self {
        Self {
            inner: Box::into_raw(Box::new(val)),
        } // todo should this be unsafe
    }
}
unsafe impl<T: Debug + ?Sized> Send for SyncSendPtrMut<T> {}
unsafe impl<T> Sync for SyncSendPtrMut<T> {}

impl<T: Debug + ?Sized> Debug for SyncSendPtrMut<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        unsafe { (*self.inner).fmt(f) }
    }
}

#[derive(Debug)]
pub(super) struct AsyncAdapter<T: ?Sized> {
    env: Arc<AsyncAdapterEnv>,
    context: SyncSendPtrMut<T>,
}

impl<T: ?Sized> AsyncAdapter<T> {
    //todo document what this is for
    fn new_internal<U: ?Sized>(&self, context_ptr: SyncSendPtrMut<U>) -> AsyncAdapter<U> {
        AsyncAdapter {
            env: self.env.clone(),
            context: context_ptr,
        }
    }

    /// Invokes the provided function with a sync method.
    async fn invoke<'c, 's, 'result, F, U>(&'s self, func: F) -> Result<U>
    where
        F: FnOnce(&'c T) -> Result<U> + Send,
        F: 'result,
        U: Send + 'result,
        's: 'result,
        'c: 'result,
        's: 'c,
    {
        // todo verify the interior mutability won't panic here
        self.env.invoke(&self.context, func).await
    }

    async fn invoke_mut<'c, 'result, F, U>(&'c self, func: F) -> Result<U>
    where
        F: FnOnce(&'c mut T) -> Result<U> + Send,
        F: 'result,
        U: Send + 'result,
        'c: 'result,
    {
        // todo verify the interior mutability won't panic here
        self.env.invoke_mut(&self.context, func).await
    }

    fn invoke_blocking<'c, 'result, F, U>(&'c self, func: F) -> Result<U>
    where
        F: FnOnce(&'c T) -> Result<U> + Send,
        F: 'result,
        U: Send + 'result,
        'c: 'result,
    {
        // todo verify the interior mutability won't panic here
        self.env.invoke_blocking(self.context.inner, func)
    }
}

impl<T> AsyncAdapter<T> {
    pub(super) fn new<F>(create_context: F) -> Result<Self>
    where
        Self: Sized,
        F: FnOnce() -> Result<T> + Send,
    {
        // TODO execute the create context function on the thread
        let context = create_context()?;
        Ok(Self {
            env: Arc::new(AsyncAdapterEnv::new()),
            context: SyncSendPtrMut::new(Box::into_raw(Box::new(context))),
        })
    }
}

impl<T: ?Sized> Drop for AsyncAdapter<T> {
    fn drop(&mut self) {
        // Drops the box to Drop T
        self.env
            .invoke_blocking(&self.context, |context| unsafe {
                std::mem::drop(Box::from_raw(context.inner));
                Ok(())
            })
            .unwrap();
        // Note, self.context.inner is now a dangling pointer
    }
}

#[async_trait(?Send)]
impl<T> ConnectionMethods for AsyncAdapter<T>
where
    T: sync::ConnectionMethods + ?Sized,
{
    async fn execute(&self, sql: &str) -> Result<()> {
        self.invoke(|conn| conn.execute(sql)).await
    }

    async fn query<'c>(
        &'c self,
        table: &str,
        columns: &[Column],
        expr: Option<BoolExpr>,
        limit: Option<i32>,
        offset: Option<i32>,
        sort: Option<&[Order]>,
    ) -> Result<RawQueryResult<'c>> {
        let rows = self
            .invoke(|conn| {
                let rows: Box<dyn BackendRows> =
                    conn.query(table, columns, expr, limit, offset, sort)?;
                let vec_rows = super::connmethods::vec_from_backend_rows(rows, columns)?;
                Ok(Box::new(vec_rows))
            })
            .await?;
        Ok(rows)
    }
    async fn insert_returning_pk(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<SqlVal> {
        self.invoke(|conn| conn.insert_returning_pk(table, columns, pkcol, values))
            .await
    }
    /// Like `insert_returning_pk` but with no return value
    async fn insert_only(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.invoke(|conn| conn.insert_only(table, columns, values))
            .await
    }
    /// Insert unless there's a conflict on the primary key column, in which case update
    async fn insert_or_replace(
        &self,
        table: &str,
        columns: &[Column],
        pkcol: &Column,
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.invoke(|conn| conn.insert_or_replace(table, columns, pkcol, values))
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
        self.invoke(|conn| conn.update(table, pkcol, pk, columns, values))
            .await
    }
    async fn delete_where(&self, table: &str, expr: BoolExpr) -> Result<usize> {
        self.invoke(|conn| conn.delete_where(table, expr)).await
    }
    /// Tests if a table exists in the database.
    async fn has_table(&self, table: &str) -> Result<bool> {
        self.invoke(|conn| conn.has_table(table)).await
    }
}

#[async_trait(?Send)]
impl<T> BackendConnection for AsyncAdapter<T>
where
    T: sync::BackendConnection,
{
    async fn transaction<'c>(&'c mut self) -> Result<Transaction<'c>> {
        let transaction_ptr: SyncSendPtrMut<dyn sync::BackendTransaction> = self
            .invoke_mut(|conn| {
                let transaction: sync::Transaction = conn.transaction()?;
                let transaction_ptr: *mut dyn sync::BackendTransaction =
                    Box::into_raw(transaction.trans);
                Ok(SyncSendPtrMut::new(transaction_ptr))
            })
            .await?;
        let transaction_adapter = self.new_internal(transaction_ptr);
        Ok(Transaction::new(Box::new(transaction_adapter)))
    }

    fn backend(&self) -> Box<dyn Backend> {
        // no sync-to-async translation needed but we still have to
        // dispatch to our worker thread because only that thread owns
        // the BackendConnection object.
        // todo clean up unwrap
        Box::new(BackendAdapter::new(
            self.invoke_blocking(|conn| Ok(conn.backend())).unwrap(),
        ))
    }
    fn backend_name(&self) -> &'static str {
        // todo clean up unwrap
        self.invoke_blocking(|conn| Ok(conn.backend_name()))
            .unwrap()
    }
    /// Tests if the connection has been closed. Backends which do not
    /// support this check should return false.
    fn is_closed(&self) -> bool {
        // todo clean up unwrap
        self.invoke_blocking(|conn| Ok(conn.is_closed())).unwrap()
    }
}

impl<T> AsyncAdapter<T>
where
    T: sync::BackendConnection + 'static,
{
    pub fn into_connection(self) -> Connection {
        Connection {
            conn: Box::new(self),
        }
    }
}

#[async_trait(?Send)]
impl<T, 'c> BackendTransaction<'c> for AsyncAdapter<T>
where
    T: sync::BackendTransaction<'c> + ?Sized,
{
    async fn commit(&mut self) -> Result<()> {
        self.invoke_mut(|conn| conn.commit()).await
    }
    async fn rollback(&mut self) -> Result<()> {
        self.invoke_mut(|conn| conn.rollback()).await
    }
    fn connection_methods(&self) -> &dyn ConnectionMethods {
        self
    }
}

#[derive(Clone)]
pub(super) struct BackendAdapter<T>
where
    T: sync::Backend + Clone,
{
    inner: T,
}
impl<T: sync::Backend + Clone> BackendAdapter<T> {
    pub(super) fn new(inner: T) -> Self {
        BackendAdapter { inner }
    }
}

#[async_trait]
impl<T: sync::Backend + Clone + 'static> Backend for BackendAdapter<T> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn create_migration_sql(&self, current: &adb::ADB, ops: Vec<adb::Operation>) -> Result<String> {
        self.inner.create_migration_sql(current, ops)
    }
    async fn connect(&self, conn_str: &str) -> Result<Connection> {
        // create a copy of the backend that can be moved into the closure
        let sync_backend: T = self.inner.clone();
        let conn_str2 = conn_str.to_string();
        tokio::task::spawn_blocking(move || {
            let connmethods_async =
                adapter::AsyncAdapter::new(|| sync_backend.connect(&conn_str2))?;
            Ok(connmethods_async.into_connection())
        })
        .await?
    }
}
