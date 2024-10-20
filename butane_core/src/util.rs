use crate::Result;
use std::sync::OnceLock;

pub fn get_or_init_once_lock<T>(cell: &OnceLock<T>, f: impl FnOnce() -> Result<T>) -> Result<&T> {
    if let Some(val) = cell.get() {
        return Ok(val);
    }
    let val = f()?;
    let _ = cell.set(val);
    match cell.get() {
        Some(val) => Ok(val),
        _ => panic!("Cell was already set, cannot be empty"),
    }
}

pub async fn get_or_init_once_lock_async<T, Fut>(
    cell: &OnceLock<T>,
    f: impl FnOnce() -> Fut,
) -> Result<&T>
where
    Fut: std::future::Future<Output = Result<T>>,
{
    if let Some(val) = cell.get() {
        return Ok(val);
    }
    let val = f().await?;
    // Note that theoretically this can block, which we shouldn't do
    // under async, but the cases when we expect multiple async jobs
    // to be operating on this are very rare (are they even allowed by the type system?).
    let _ = cell.set(val);
    match cell.get() {
        Some(val) => Ok(val),
        _ => panic!("Cell was already set, cannot be empty"),
    }
}
