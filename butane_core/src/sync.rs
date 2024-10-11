use crate::{Error, Result};
use tokio::sync::OnceCell;

/// Wrapper around tokio's OnceCell get_or_try_init method
/// This is a sync version which provides the same semantics with a spinlock, as simultaneous initialization should be very rare.
pub fn get_or_try_init_tokio_once_cell_sync<T, F>(cell: &OnceCell<T>, f: F) -> Result<&T>
where
    F: Fn() -> Result<T>,
{
    match cell.get() {
        Some(val) => Ok(val),
        None => {
            loop {
                match cell.set(f()?) {
                    Ok(()) => break,
                    Err(tokio::sync::SetError::AlreadyInitializedError(_)) => break,
                    Err(tokio::sync::SetError::InitializingError(_)) => continue, // spinlock
                }
            }
            // Error should be impossible here, we should have already ensured init (or returned error).
            cell.get().ok_or(Error::NotInitialized)
        }
    }
}
