* [x] Clean up pattern for sync/async variants. Inconsistent between suffix and module
* [x] Tests should run against sync and async
* [ ] Establish soundness for unsafe sections of AsyncAdapter
* [ ] Consider publishing `AsyncAdapter` into its own crate
* [x] Ensure Postgres works in sync
* [x] Re-enable R2D2 for sync
* [ ] Integrate deadpool or bb8 for async connection poll
* [x] Fix `#[async_trait(?Send)]` to set up Send bound again as it's required for e.g. `tokio::spawn`
* [ ] Separate sync and async examples
* [ ] Should async_adapter be under a separate feature? Do we need it for migrations?
* [x] Ensure sqlite works in async
* [x] Fully support sync too. Using async should not be required
* [ ] Clean up miscellaneous TODOs
