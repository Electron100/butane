# Trybuild tests

This crate contains compile-fail tests using [trybuild](https://docs.rs/trybuild/latest/trybuild/)
to verify that Butane macros produce clear, helpful error messages when used incorrectly.

These tests snapshot compiler errors and warnings to ensure that:

- Incorrect macro usage fails at compile time as expected
- Error messages are clear and actionable for developers
- Changes to macros don't accidentally worsen error messages

## Test Structure

- `tests/trybuild/pass/` - Tests that should compile successfully
- `tests/trybuild/fail/` - Tests that should fail to compile with specific error messages
- `tests/trybuild/should-fail/` - Known issues: tests that should fail, but currently pass

## Running Tests

```bash
cargo test -p trybuild_tests
```

To update snapshots when intentionally changing error messages:

```bash
TRYBUILD=overwrite cargo test -p trybuild_tests
```

See [CONTRIBUTING.md](../CONTRIBUTING.md) for more information on contributing to Butane.
