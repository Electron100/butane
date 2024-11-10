* [x] Clean up pattern for sync/async variants. Inconsistent between suffix and module
* [x] Tests should run against sync and async
* [x] Ensure Postgres works in sync
* [x] Re-enable R2D2 for sync
* [x] Fix `#[async_trait(?Send)]` to set up Send bound again as it's required for e.g. `tokio::spawn`
* [x] Separate sync and async examples
* [x] Ensure sqlite works in async
* [x] Fully support sync too. Using async should not be required
* [ ] Clean up miscellaneous TODOs
* [x] Establish soundness for unsafe sections of AsyncAdapter
* [ ] Should async and/or async_adapter be under a separate feature?
* [ ] Integrate deadpool or bb8 for async connection pool
