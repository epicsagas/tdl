# Contributing to tdl

Thank you for your interest in contributing! This document covers the development workflow and coding standards.

## Getting Started

```bash
# Clone the repository
git clone https://github.com/epicsagas/tdl.git
cd tdl

# Build
cargo build --release

# Run tests
cargo test --lib

# Run linter
cargo clippy --features gui -- -D warnings
```

## Development Workflow

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests and linter
5. Commit your changes (see [Commit Format](#commit-format))
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

## Code Style

- Rust 2024 edition
- `snake_case` for functions/variables/modules
- `PascalCase` for types/enums/structs
- Use `anyhow::Result` for fallible functions
- Add `.context()` to errors for better debugging
- Use `tracing` macros for logging (not `println!` in library code)

## Testing

Run the test suite:

```bash
# All tests
cargo test --lib

# Specific module
cargo test --lib pathfmt

# With output
cargo test --lib -- --nocapture
```

## Commit Format

Follow conventional commits:

- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation only
- `refactor:` - Code change that neither fixes a bug nor adds a feature
- `perf:` - Performance improvement
- `test:` - Adding or updating tests

Example: `feat(dl): add progress bar for downloads`

## Pull Request Guidelines

- Describe what the PR does and why
- Link related issues
- Ensure all tests pass
- Update documentation if needed
- Keep PRs focused and manageable in size

## Questions?

Open an issue with the `question` label.
