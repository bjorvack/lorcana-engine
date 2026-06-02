# Contributing to lorcana-engine

Thank you for your interest in contributing to lorcana-engine! This document provides guidelines and instructions for contributing to the project.

## Development Setup

### Prerequisites

- Rust latest stable version (use `rustup update stable` to ensure you have the latest)
- Cargo (comes with Rust)
- Git

### Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd lorcana-engine
```

2. Install Rust toolchain components:
```bash
rustup component add rustfmt clippy
```

3. Install Lefthook for git hooks (optional but recommended):
```bash
# On macOS with Homebrew
brew install lefthook

# On Linux
curl -1sLf 'https://github.com/evilmartians/lefthook/releases/download/v1.7.2/lefthook_1.7.2_Linux_x86_64.tar.gz' | tar -xz
sudo mv lefthook /usr/local/bin/

# Or install via cargo
cargo install lefthook
```

4. Install git hooks:
```bash
lefthook install
```

## Development Workflow

### Making Changes

1. Create a new branch for your changes:
```bash
git checkout -b feature/your-feature-name
```

2. Make your changes and write tests for new functionality.

3. Run the development checks:
```bash
# Format code
cargo fmt --all

# Run linter
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test --all-targets --all-features
```

### Git Hooks

This project uses Lefthook to automate quality checks before commits and pushes:

- **Pre-commit**: Runs `cargo fmt --check`, `cargo clippy`, and `cargo test`
- **Pre-push**: Runs the same checks as pre-commit

If you have Lefthook installed, these hooks will run automatically. If not, you can run them manually before committing.

### Committing Changes

This project follows conventional commits and requires atomic commits. All commits must pass linting and tests before being committed.

Commit message format:
```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks
- `perf`: Performance improvements

Example:
```
feat(card): add card parsing functionality

Implement the card parser for Lorcana card data.
This includes parsing card name, cost, ink, and abilities.
```

## Code Style and Quality

### Formatting

All code must be formatted with `rustfmt`. The project uses a custom `rustfmt.toml` configuration:

```bash
cargo fmt --all
```

To check formatting without making changes:
```bash
cargo fmt --all -- --check
```

### Linting

This project uses Clippy with strict linting rules configured in `Cargo.toml` and `clippy.toml`:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

The linter is configured to:
- Enable all Clippy lints (`clippy::all`)
- Enable pedantic lints (`clippy::pedantic`)
- Enable nursery lints (`clippy::nursery`)
- Enable cargo lints (`clippy::cargo`)
- Treat warnings as errors (`-D warnings`)

### Testing

All changes must include tests. Run tests with:

```bash
cargo test --all-targets --all-features
```

Test organization:
- Unit tests should be placed in the same module as the code they test
- Integration tests should be placed in the `tests/` directory
- Use descriptive test names that explain what is being tested

### Documentation

Public APIs must be documented with rustdoc comments:

```rust
/// Parses a card from the given input string.
///
/// # Arguments
///
/// * `input` - A string slice containing the card data
///
/// # Returns
///
/// Returns a `Result` containing the parsed `Card` or an error
///
/// # Examples
///
/// ```
/// let card = parse_card("...").unwrap();
/// ```
pub fn parse_card(input: &str) -> Result<Card> {
    // ...
}
```

Build documentation with:
```bash
cargo doc --no-deps --document-private-items
```

## Project Structure

```
lorcana-engine/
├── src/
│   ├── main.rs          # Main entry point
│   └── ...              # Other source files
├── tests/               # Integration tests
├── Cargo.toml           # Project configuration
├── rustfmt.toml         # Formatting configuration
├── clippy.toml          # Linter configuration
├── lefthook.yml         # Git hooks configuration
└── CONTRIBUTING.md      # This file
```

## Best Practices

### General Rust Guidelines

- Use Rust 2024 edition features where appropriate
- Prefer `&str` over `String` for function arguments when ownership isn't needed
- Use `Result` for error handling, avoid `unwrap()` in production code
- Leverage Rust's type system for correctness
- Use iterators and functional patterns over imperative loops when appropriate
- Keep functions small and focused
- Use meaningful variable and function names

### Error Handling

Use the `?` operator for error propagation:

```rust
fn process_card(input: &str) -> Result<Card, ParseError> {
    let data = parse_input(input)?;
    let card = build_card(data)?;
    Ok(card)
}
```

### Testing

- Write tests for all public functions
- Use table-driven tests for multiple test cases
- Test edge cases and error conditions
- Keep tests fast and focused

### Performance

- Profile before optimizing
- Use `cargo bench` for benchmarking
- Consider using `--release` for performance testing
- Be mindful of allocations and copies

## Submitting Changes

1. Ensure all tests pass: `cargo test`
2. Ensure code is formatted: `cargo fmt --all -- --check`
3. Ensure no linter warnings: `cargo clippy --all-targets --all-features -- -D warnings`
4. Update documentation if needed
5. Push your branch and create a pull request

## Getting Help

If you need help:
- Open an issue for bugs or feature requests
- Start a discussion for questions
- Check existing issues and discussions first

## License

By contributing to this project, you agree that your contributions will be licensed under the same license as the project.