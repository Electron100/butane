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
                        return; // break out of the loop
                    }
                }
            }
        });
        Self {
            sender,
            thread_handle: Some(thread_handle),
        }
    }

    /// Invokes a blocking function `func` as if it were async.
    ///
    /// This is implemented by running it on the special thread created
    /// when the `AsyncAdapterEnv` was created.
    async fn invoke<'c, 's, 'result, F, T, U>(
        &'s self,
        context: &SyncSendPtrMut<T>,
        func: F,
    ) -> Result<U>
    where
        F: FnOnce(&'c T) -> Result<U> + Send,
        F: 'result,
        U: Send + 'result,
        T: ?Sized + 'c,
        's: 'result,
        'c: 'result,
    {
        // func itself must be `Send`, but we do not require &T to be
        // Send (and thus don't require T to be Sync).  We do this by
        // basically unsafely sending our raw context pointer over to
        // the worker thread anyway.  The key observation on why we
        // believe this to be sound is that we actually created the
        // context over on the worker thread in the first place (see
        // [AsyncAdapter::new]) and we do not allow direct access to
        // it. So despite fact that we pass the context pointer back
        // and forth, it's essentially owned by the worker thread -- all operations
        // with context occur on that worker thread.
        let (tx, rx) = tokio::sync::oneshot::channel();
        let func_taking_ptr = |ctx: SyncSendPtrMut<T>| func(unsafe { ctx.inner.as_ref() }.unwrap());
        unsafe {
            let wrapped_func = move || _ = tx.send(func_taking_ptr(context.clone_unsafe()));
            self.invoke_internal_unsafe(wrapped_func)?;
        }
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
        T: ?Sized + 'c,
        's: 'result,
        'c: 'result,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let func_taking_ptr = |ctx: SyncSendPtrMut<T>| func(unsafe { ctx.inner.as_mut().unwrap() });
        unsafe {
            let wrapped_func = move || _ = tx.send(func_taking_ptr(context.clone_unsafe()));
            self.invoke_internal_unsafe(wrapped_func)?;
        }
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
        let context_ptr = unsafe { SendPtr::new(context) };
        let func_taking_ptr = |ctx: SendPtr<T>| func(unsafe { ctx.inner.as_ref() }.unwrap());
        unsafe {
            let wrapped_func = move || _ = tx.send(func_taking_ptr(context_ptr));
            self.invoke_internal_unsafe(wrapped_func)?;
        }
        rx.recv()?
    }

    unsafe fn invoke_internal_unsafe<'s, 'result>(
        &'s self,
        // wrapped_func is a complete encapsulation of the function we
        // want to invoke without any parameters left to provide
        wrapped_func: impl FnOnce() + Send + 'result,
    ) -> Result<()> {
        // We transmute wrapped_func (with an intermediate boxing
        // step) solely to transform it's lifetime. The lifetime is an
        // issue here because Rust itself has no way of knowing how
        // long our sync worker thread is going to use it for.  But
        // *we* know that our worker thread will immediately execute
        // the function and the caller to this method will wait to
        // hear from the worker thread before proceeding (and thus
        // before letting the lifetime lapse)
        // https://stackoverflow.com/questions/52424449/
        let boxed_func: Box<dyn FnOnce() + Send + 'result> = Box::new(wrapped_func);
        let static_func: Box<dyn FnOnce() + Send + 'static> =
            unsafe { std::mem::transmute(boxed_func) };
        self.sender.send(Command::Func(static_func))?;
        Ok(())
    }
}

impl Drop for AsyncAdapterEnv {
    fn drop(&mut self) {
        let r = self.sender.send(Command::Shutdown);
        if r.is_err() {
            // editorconfig-checker-disable
            crate::error!("Cannot send async adapter env shutdown command because channel is disconnected.\
                           Assuming this means thread died and is joinable. If it is not, join may hang indefinitely");
            // editorconfig-checker-enable
        }
        self.thread_handle.take().map(|h| h.join());
    }
}

/// Wrapper around a raw pointer that we assert is [`Send`].
///
/// Needless to say, this requires care. See comments on `AsyncAdapterEnv::invoke`.
/// for why we believe this to be sound.
struct SendPtr<T: ?Sized> {
    inner: *const T,
}
impl<T: ?Sized> SendPtr<T> {
    unsafe fn new(inner: *const T) -> Self {
        Self { inner }
    }
}
unsafe impl<T: ?Sized> Send for SendPtr<T> {}

/// Like [`SendPtrMut`] but we also assert that it is [`Sync`].
struct SyncSendPtrMut<T: ?Sized> {
    inner: *mut T,
}
impl<T: ?Sized> SyncSendPtrMut<T> {
    unsafe fn new(inner: *mut T) -> Self {
        Self { inner }
    }
    unsafe fn clone_unsafe(&self) -> Self {
        Self { inner: self.inner }
    }
}
impl<T> From<T> for SyncSendPtrMut<T>
where
    T: Sized,
{
    fn from(val: T) -> Self {
        Self {
            inner: Box::into_raw(Box::new(val)),
        }
    }
}
unsafe impl<T: ?Sized> Send for SyncSendPtrMut<T> {}
unsafe impl<T: ?Sized> Sync for SyncSendPtrMut<T> {}

impl<T: Debug + ?Sized> Debug for SyncSendPtrMut<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        // We enforce that inner is non-null and valid.
        unsafe { (*self.inner).fmt(f) }
    }
}

#[derive(Debug)]
pub(super) struct AsyncAdapter<T: ?Sized> {
    env: Arc<AsyncAdapterEnv>,
    context: SyncSendPtrMut<T>,
}

impl<T: ?Sized> AsyncAdapter<T> {
    /// Create an `AsyncAdapter` with the given context, using the same `env` as self.
    fn create_with_same_env<U: ?Sized>(&self, context_ptr: SyncSendPtrMut<U>) -> AsyncAdapter<U> {
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
        self.env.invoke(&self.context, func).await
    }

    async fn invoke_mut<'c, 'result, F, U>(&'c self, func: F) -> Result<U>
    where
        F: FnOnce(&'c mut T) -> Result<U> + Send,
        F: 'result,
        U: Send + 'result,
        'c: 'result,
    {
        self.env.invoke_mut(&self.context, func).await
    }

    fn invoke_blocking<'c, 'result, F, U>(&'c self, func: F) -> Result<U>
    where
        F: FnOnce(&'c T) -> Result<U> + Send,
        F: 'result,
        U: Send + 'result,
        'c: 'result,
    {
        self.env.invoke_blocking(self.context.inner, func)
    }
}

impl<T> AsyncAdapter<T> {
    /// Create an async adapter using `create_context` to create an instance of the inner type `T`.
    pub(super) fn new<F>(create_context: F) -> Result<Self>
    where
        Self: Sized,
        F: FnOnce() -> Result<T> + Send,
    {
        let env = AsyncAdapterEnv::new();

        // Execute the context creation function on our worker thread.
        let dummy = (); // because we have to pass a context pointer to env.invoke
        let context = env.invoke_blocking(&dummy, |_ctx: &()| {
            let concrete_context = create_context()?;
            // See comments about soundness on AsyncAdapterEnv::invoke
            let context = unsafe { SyncSendPtrMut::new(Box::into_raw(Box::new(concrete_context))) };
            Ok(context)
        })?;

        Ok(Self {
            env: Arc::new(env),
            context,
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

#[async_trait]
impl<T> ConnectionMethodsAsync for AsyncAdapter<T>
where
    T: ConnectionMethods + ?Sized,
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
    /// Like `insert_returning_pk` but with no return value.
    async fn insert_only(
        &self,
        table: &str,
        columns: &[Column],
        values: &[SqlValRef<'_>],
    ) -> Result<()> {
        self.invoke(|conn| conn.insert_only(table, columns, values))
            .await
    }
    /// Insert unless there's a conflict on the primary key column, in which case update.
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

#[async_trait]
impl<T> BackendConnectionAsync for AsyncAdapter<T>
where
    T: BackendConnection,
{
    async fn transaction<'c>(&'c mut self) -> Result<TransactionAsync<'c>> {
        let transaction_ptr: SyncSendPtrMut<dyn BackendTransaction> = self
            .invoke_mut(|conn| {
                let transaction: Transaction = conn.transaction()?;
                let transaction_ptr: *mut dyn BackendTransaction = Box::into_raw(transaction.trans);
                Ok(unsafe { SyncSendPtrMut::new(transaction_ptr) })
            })
            .await?;
        let transaction_adapter = self.create_with_same_env(transaction_ptr);
        Ok(TransactionAsync::new(Box::new(transaction_adapter)))
    }

    fn backend(&self) -> Box<dyn Backend> {
        ok_or_panic_with_adapter_error(self.invoke_blocking(|conn| Ok(conn.backend())))
    }

    fn backend_name(&self) -> &'static str {
        ok_or_panic_with_adapter_error(self.invoke_blocking(|conn| Ok(conn.backend_name())))
    }

    /// Tests if the connection has been closed.
    ///
    /// Backends which do not support this check should return `false`.
    fn is_closed(&self) -> bool {
        ok_or_panic_with_adapter_error(self.invoke_blocking(|conn| Ok(conn.is_closed())))
    }
}

fn ok_or_panic_with_adapter_error<T>(r: Result<T>) -> T {
    match r {
        Ok(ret) => ret,
        // This is unfortunate, but should be rare. We never use it
        // when invoking functions that can fail in their own right,
        // so it indicates that the channel operation failed, which
        // should only be possible if the other thread died
        // unexpectedly.
        Err(e) => panic!(
            // editorconfig-checker-disable
            "Internal error occurred within the sync->async adapter invoked when wrapping a function\
             which does not permit error returns.\n\
             Error: {}",
            // editorconfig-checker-enable
        e
        )
    }
}

impl<T> AsyncAdapter<T>
where
    T: BackendConnection + 'static,
{
    pub fn into_connection(self) -> ConnectionAsync {
        ConnectionAsync {
            conn: Box::new(self),
        }
    }
}

#[async_trait]
impl<T, 'c> BackendTransactionAsync<'c> for AsyncAdapter<T>
where
    T: BackendTransaction<'c> + ?Sized,
{
    async fn commit(&mut self) -> Result<()> {
        self.invoke_mut(|conn| conn.commit()).await
    }
    async fn rollback(&mut self) -> Result<()> {
        self.invoke_mut(|conn| conn.rollback()).await
    }
    fn connection_methods(&self) -> &dyn ConnectionMethodsAsync {
        self
    }
}

/// Create an async connection via the synchronous `connect` method of `backend`.
///
/// Use this when authoring a backend which doesn't natively support async.
pub async fn connect_async_via_sync<B>(backend: &B, conn_str: &str) -> Result<ConnectionAsync>
where
    B: Backend + Clone + 'static,
{
    // create a copy of the backend that can be moved into the closure
    let backend2 = backend.clone();
    let conn_str2 = conn_str.to_string();
    tokio::task::spawn_blocking(move || {
        let connmethods_async = adapter::AsyncAdapter::new(|| backend2.connect(&conn_str2))?;
        Ok(connmethods_async.into_connection())
    })
    .await?
}
