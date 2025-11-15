# Contributing to Butane

Thank you for your interest in contributing to Butane!
This guide will help you set up your development environment and get started.

## Development Environment Setup

### Using mise (Recommended)

This project uses [mise](https://mise.jdx.dev/) to manage development tools needed for development.

#### Installing mise

**Package Managers (Recommended):**

mise is available in many package managers. Choose the one for your system:

- **Nix/NixOS:** `nix-env -iA nixpkgs.mise`
- **macOS (Homebrew):** `brew install mise`
- **macOS (MacPorts):** `sudo port install mise`
- **Windows (Chocolatey):** `choco install mise-en-place`
- **Windows (Scoop):** `scoop install mise`
- **Arch Linux:** `pacman -S mise`
- **Alpine Linux:** `apk add mise`
- **FreeBSD:** `pkg install mise`

For a complete list of package managers and distributions,
see [mise on Repology](https://repology.org/project/mise/versions).

##### Alternative: Install via Cargo

If you already have Rust installed, you can install mise using cargo:

```bash
cargo install mise
```

**Note:** When installing via cargo, you'll need to manually set up its shell integration.

##### Alternative: Quick Install Script

> ⚠️ **Security Warning:** This alternative is not secure. We recommend using a package manager instead.

If you prefer the installation script:

**macOS/Linux:**

```bash
curl https://mise.run | sh
```

**Windows (PowerShell):**

```powershell
irm https://mise.run | iex
```

**For other installation methods**, see the [official mise documentation](https://mise.jdx.dev/getting-started.html).

#### Installing Project Tools

Once mise is installed and shell integration is activated, navigate to the project directory and run:

```bash
mise install
```

This will automatically install all required tools defined in `.mise.toml`.

### Without mise

If you prefer not to use mise, you'll need to manually install:

1. **Rust** (stable) - via [rustup](https://rustup.rs/)
2. **PostgreSQL** - for running tests
3. **SQLite** - for running tests
4. **Make** - for running development commands
5. Other development tools as needed - they will typically be added to `.mise.toml`.

## Building the Project

```bash
cargo build
```

## Running Tests

Run all tests:

```bash
cargo test
```

It is also recommended to use cargo-nextest which provides many benefits, especially when running all tests:

```bash
cargo nextest run
```

Note cargo-nextest does not support doctests, [yet](https://github.com/nextest-rs/nextest/issues/16).

To check those:

```bash
cargo test -p butane --doc --features async
cargo test -p butane_codegen --doc  # all doctest are ignored
```

Run tests for a specific package:

```bash
cargo test -p butane_core --all-features
cargo test -p butane_tests --all-features
cargo test -p trybuild_tests
```

If you are enhancing `butane_codegen`, please create tests in [`trybuild_tests`](trybuild_tests/)
instead of `butane_codegen`.

## Development notes

There is a cyclic test dependency between `butane_test_helper` and `butane_core`.

`butane_test_helper` uses `butane_core` in its library.

`butane_core` uses `butane_test_helper` in its tests.

## Code Quality

Before submitting a PR, you may use the following commands locally to ensure that your code passes all checks:

```bash
make check
```

Or run individual checks:

```bash
# Format code
make fmt

# Run linter
make lint

# Check for typos
make spellcheck

# Check formatting and editor config
make check-fmt

# Check documentation
make doclint
```

## Project Structure

- `butane/` - Main library crate
- `butane_core/` - Core functionality
- `butane_cli/` - Command-line interface
- `butane_codegen/` - Code generation macros
- `butane_test_helper/` - Test utilities
- `examples/` - Example projects
- `docs/` - Documentation

## Database Migrations

This repository contains generated migrations for the examples. To regenerate migrations:

```bash
make regenerate-example-migrations
```

## Submitting Changes

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Make your changes
4. Run tests and quality checks
5. Commit your changes with clear commit messages
6. Push to your fork
7. Open a Pull Request

## Getting Help

- Open an issue on [GitHub](https://github.com/Electron100/butane/issues)
- Check existing issues and PRs
- Review the [documentation](https://docs.rs/butane)

Thank you for contributing to Butane!
