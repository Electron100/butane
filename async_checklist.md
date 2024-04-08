* [ ] Fully support sync too. Using async should not be required
* [ ] Clean up pattern for sync/async variants. Inconsistent between suffix and module
* [ ] Tests should run against sync and async
* [ ] Establish soundness for unsafe sections of AsyncAdapter
* [ ] Consider publishing `AsyncAdapter` into its own crate
* [ ] Ensure Postgres works in sync
* [ ] Ensure sqlite works in async (might already be done)
* [ ] Re-enable R2D2 for sync and find async alternative (deadpool)
* [ ] Fix `#[async_trait(?Send)]` to set up Send bound again as it's required for e.g. `tokio::spawn`

