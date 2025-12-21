# Contributing to Rustun

[中文版](./CONTRIBUTING_CN.md) | English

Thank you for your interest in contributing to Rustun! We welcome contributions from everyone.

## 📋 Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How Can I Contribute?](#how-can-i-contribute)
- [Development Setup](#development-setup)
- [Coding Guidelines](#coding-guidelines)
- [Commit Messages](#commit-messages)
- [Pull Request Process](#pull-request-process)
- [Reporting Bugs](#reporting-bugs)
- [Suggesting Enhancements](#suggesting-enhancements)

## Code of Conduct

This project follows a Code of Conduct that all contributors are expected to adhere to. Please be respectful and constructive in all interactions.

## How Can I Contribute?

### Reporting Bugs

If you find a bug, please create an issue with:

- **Clear title and description**
- **Steps to reproduce** the problem
- **Expected behavior** vs **actual behavior**
- **Environment details** (OS, Rust version, etc.)
- **Relevant logs** or error messages

### Suggesting Enhancements

We welcome feature requests! Please:

- Check if the feature is already requested in Issues
- Provide a clear use case and description
- Explain why this feature would be useful
- Consider implementation approaches

### Code Contributions

1. **Bug fixes** - Always welcome!
2. **New features** - Please discuss in an issue first
3. **Documentation** - Improvements to docs are highly valued
4. **Tests** - Adding test coverage is greatly appreciated

## Development Setup

### Prerequisites

- Rust 1.70 or higher
- Git
- Docker (for cross-compilation testing)

### Setup Steps

```bash
# Clone the repository
git clone https://github.com/smartethnet/rustun.git
cd rustun

# Build the project
cargo build

# Run tests
cargo test

# Run lints
cargo clippy -- -D warnings

# Format code
cargo fmt
```

### Development Tools

We recommend installing these tools:

```bash
# Auto-reload on file changes
cargo install cargo-watch

# Better test output
cargo install cargo-nextest

# Dependency management
cargo install cargo-edit
```

### Running in Development

```bash
# Run server with auto-reload
cargo watch -x 'run --bin server -- etc/server.toml'

# Run client with auto-reload
cargo watch -x 'run --bin client -- -s 127.0.0.1:8080 -i test-client'
```

## Coding Guidelines

### Rust Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for consistent formatting
- Ensure `cargo clippy` passes without warnings
- Write idiomatic Rust code

### Code Organization

```
src/
├── client/          # Client-side code
├── server/          # Server-side code
├── codec/           # Protocol encoding/decoding
├── crypto/          # Encryption implementations
├── network/         # Network abstractions
└── utils/           # Shared utilities
```

### Best Practices

1. **Error Handling**
   - Use `Result` and `?` operator
   - Provide meaningful error messages
   - Avoid `unwrap()` in production code

2. **Documentation**
   - Add doc comments for public APIs
   - Include examples in documentation
   - Update README.md when adding features

3. **Testing**
   - Write unit tests for new functions
   - Add integration tests for features
   - Ensure tests pass before submitting

4. **Performance**
   - Avoid unnecessary allocations
   - Use async/await properly
   - Profile before optimizing

### Code Example

```rust
/// Encrypts data using the configured cipher
///
/// # Arguments
/// * `data` - The plaintext data to encrypt
///
/// # Returns
/// * `Ok(Vec<u8>)` - The encrypted ciphertext
/// * `Err(CryptoError)` - If encryption fails
///
/// # Example
/// ```
/// let encrypted = encrypt(b"hello")?;
/// ```
pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    // Implementation
}
```

## Commit Messages

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

### Examples

```bash
# Good commit messages
feat(client): add keepalive configuration options
fix(server): handle connection timeout correctly
docs(readme): update installation instructions
refactor(crypto): simplify encryption interface

# Bad commit messages
fix bug
update code
changes
```

### Commit Message Guidelines

- Use present tense ("add" not "added")
- Use imperative mood ("move" not "moves")
- Limit first line to 72 characters
- Reference issues with `#issue-number`

## Pull Request Process

### Before Submitting

1. **Fork** the repository
2. **Create a branch** from `main`:
   ```bash
   git checkout -b feat/my-feature
   # or
   git checkout -b fix/issue-123
   ```

3. **Make your changes**
   - Follow coding guidelines
   - Add tests
   - Update documentation

4. **Test your changes**
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

5. **Commit your changes**
   - Follow commit message guidelines
   - Make logical, atomic commits

### Submitting PR

1. **Push** to your fork:
   ```bash
   git push origin feat/my-feature
   ```

2. **Open a Pull Request** with:
   - Clear title and description
   - Reference related issues
   - List changes made
   - Screenshots (if UI changes)

3. **PR Template**:
   ```markdown
   ## Description
   Brief description of changes

   ## Motivation
   Why this change is needed

   ## Changes
   - Change 1
   - Change 2

   ## Testing
   How was this tested?

   ## Related Issues
   Fixes #123
   ```

### Review Process

- Maintainers will review your PR
- Address review comments
- Keep PR updated with `main`
- Be patient and respectful

### After Approval

- PR will be merged by maintainers
- Delete your branch after merge
- Celebrate your contribution! 🎉

## Project Structure

```
rustun/
├── src/
│   ├── client/         # Client implementation
│   ├── server/         # Server implementation
│   ├── codec/          # Frame encoding/decoding
│   ├── crypto/         # Encryption modules
│   ├── network/        # Network abstractions
│   └── utils/          # Utilities
├── doc/                # Documentation
├── etc/                # Configuration examples
├── build.sh            # Build script
└── tests/              # Integration tests
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Run ignored tests
cargo test -- --ignored
```

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Arrange
        let input = "test";
        
        // Act
        let result = my_function(input);
        
        // Assert
        assert_eq!(result, expected);
    }
}
```

## Documentation

### Updating Documentation

- **README.md** - User-facing documentation
- **Doc comments** - API documentation
- **BUILD.md** - Build instructions
- **CHANGELOG.md** - Track changes

### Building Docs

```bash
# Generate and open documentation
cargo doc --open

# Check documentation
cargo doc --no-deps
```

## Release Process

(For maintainers only)

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Create release tag
4. Build binaries with `./build.sh`
5. Create GitHub Release
6. Upload binaries

## Getting Help

- **GitHub Issues**: Bug reports and features
- **GitHub Discussions**: Questions and ideas
- **Documentation**: Check README and docs

## Recognition

Contributors will be:
- Listed in release notes
- Acknowledged in CONTRIBUTORS file
- Appreciated by the community!

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

**Thank you for contributing to Rustun!** ❤️

