# Contributing to Skylet Execution Engine

Thank you for your interest in contributing! This document provides guidelines and instructions for contributing to the Skylet Execution Engine.

## Code of Conduct

We are committed to providing a welcoming and inspiring community for all. Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Getting Started

### Prerequisites

- Rust 1.70.0 or later (via [rustup](https://rustup.rs/))
- Git
- Familiarity with Rust and async/await

### Development Setup

```bash
# Clone the repository
git clone https://github.com/vincents-ai/skylet.git
cd skylet/execution-engine

# Verify build
cargo check --features standalone

# Run tests
cargo test --lib

# Build documentation
cargo doc --no-deps --open
```

## Development Workflow

### 1. Fork and Branch

```bash
# Create feature branch from main
git checkout -b feature/your-feature-name

# Use descriptive branch names:
# - feature/new-capability
# - fix/bug-description  
# - docs/improvement
# - refactor/module-name
```

### 2. Make Changes

Follow these guidelines:

#### Code Style
- Use snake_case for functions and variables
- Use PascalCase for types and traits
- Use SCREAMING_SNAKE_CASE for constants
- Line length: 100 columns preferred (soft limit)
- Use meaningful variable names

```rust
// Good
fn process_configuration(config: &Config) -> Result<()> {
    let max_retries = config.max_retries.unwrap_or(3);
    // ...
}

// Avoid
fn proc(c: &Config) -> Result<()> {
    let mr = c.mr.unwrap_or(3);
    // ...
}
```

#### Imports Organization
Group imports with blank lines between:
```rust
// Standard library
use std::sync::Arc;
use std::time::Duration;

// External crates
use anyhow::Result;
use tokio::sync::RwLock;
use serde_json::json;

// Local modules
use crate::config::AppConfig;
use crate::plugin_manager::PluginManager;
```

#### Documentation
- Add doc comments to public items:

```rust
/// Processes a request through the plugin pipeline.
///
/// # Arguments
/// * `request` - The incoming request
/// * `context` - Plugin execution context
///
/// # Returns
/// Result containing the response or error
///
/// # Examples
/// ```
/// let response = process_request(&request, &context)?;
/// ```
pub async fn process_request(
    request: &Request,
    context: &PluginContext,
) -> Result<Response> {
    // Implementation
}
```

#### Error Handling
- Use `anyhow::Result<T>` for most functions
- Use `thiserror` for custom error types in libraries
- Provide context in errors:

```rust
// Good
let config = load_config(path)
    .context(format!("Failed to load config from {:?}", path))?;

// Avoid
let config = load_config(path)?;
```

### 3. Testing

Add tests for all changes:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_works() {
        // Test setup
        let input = create_test_input();
        
        // Execute
        let result = my_function(&input);
        
        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_async_feature() {
        // Async test
        let result = async_function().await;
        assert_eq!(result, expected_value);
    }
}
```

#### Test Guidelines
- Test both success and error cases
- Use descriptive test names
- Keep tests focused on single behavior
- Target 80%+ code coverage
- Run: `cargo test --lib`

### 4. Documentation

Update documentation for user-facing changes:

- Update relevant guide in `docs/` directory
- Update inline code comments
- Update README if needed
- Add examples to doc comments

### 5. Commit Changes

Write clear, conventional commits:

```bash
# Format: type(scope): message

git commit -m "feat(config): add duration field type support

- Implement Duration field type in ConfigFieldType enum
- Add validation rules for duration format
- Update configuration reference documentation
- Add tests for duration parsing"
```

#### Commit Types
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `perf`: Performance improvement
- `chore`: Maintenance (deps, cleanup)
- `security`: Security improvement

#### Commit Guidelines
- One logical change per commit
- Descriptive messages
- Reference issues: `Closes #123`
- Sign commits: `git commit -S`

### 6. Prepare for Review

Before pushing, verify:

```bash
# Run all checks
cargo check --features standalone
cargo test --lib
cargo clippy -- -D warnings
cargo fmt

# Build documentation
cargo doc --no-deps

# Run security audit
cargo audit
```

### 7. Push and Create PR

```bash
# Push branch
git push origin feature/your-feature-name

# Create pull request on GitHub
# - Use clear title
# - Reference related issues
# - Describe changes and motivation
# - Link to documentation
```

## Pull Request Guidelines

### PR Title
- Use conventional commit format
- Be specific and descriptive
- Examples:
  - `feat(config): add duration field type`
  - `fix(plugin): prevent memory leak on unload`
  - `docs: improve security guide`

### PR Description Template

```markdown
## Description
Brief description of the change.

## Motivation
Why is this change needed?

## Changes
- Change 1
- Change 2
- Change 3

## Breaking Changes
If any, describe them here.

## Testing
How was this tested?

## Documentation
What documentation was updated?

## Checklist
- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] Code follows style guidelines
- [ ] No breaking changes (or documented)
- [ ] Commit messages are clear
- [ ] No new warnings from clippy
```

## Review Process

### Code Review
- At least one maintainer review required
- Address feedback with clarifying comments
- Request re-review after changes
- Maintainers may ask for improvements

### Approval Criteria
- ✅ Passes all tests
- ✅ No clippy warnings
- ✅ Documentation updated
- ✅ Follows code style
- ✅ Addresses issue or improves codebase
- ✅ No security concerns

## Architecture Guidelines

### Module Organization

```
src/
├── lib.rs              # Public API exports
├── config.rs           # Configuration
├── plugin_manager.rs   # Plugin management
├── service_registry.rs # Service registry
└── utils/
    └── mod.rs
```

### Trait Abstractions
Prefer traits for:
- External service integration
- Pluggable implementations
- Testing and mocking

```rust
pub trait Logger {
    fn log(&self, message: &str);
}

pub struct DefaultLogger;
impl Logger for DefaultLogger {
    fn log(&self, message: &str) {
        println!("{}", message);
    }
}
```

### Async Patterns
- Use `tokio::spawn` for background tasks
- Use `tokio::select!` for concurrent operations
- Avoid blocking in async context
- Document async behavior

## Security Considerations

### Before Submitting
- [ ] No hardcoded secrets
- [ ] Input validation on boundaries
- [ ] No unsafe code without justification
- [ ] Cryptographic operations reviewed
- [ ] Dependencies audited: `cargo audit`

### Sensitive Areas
Code that touches these areas needs extra review:
- Cryptographic operations
- FFI boundaries
- Memory management
- Configuration/secrets
- User input validation

## Performance Guidelines

### Optimization
- Profile before optimizing
- Use flamegraph: `cargo flamegraph`
- Benchmark changes: `cargo bench`
- Document performance implications

### Acceptable Tradeoffs
- Readability > micro-optimizations
- Safety > minor performance gains
- Maintainability > premature optimization

## Documentation Standards

### Required Documentation
- Public functions and types
- Non-obvious logic
- Async behavior
- Error cases
- Security considerations

### Example Documentation

```rust
/// Validates and loads a plugin configuration.
///
/// # Arguments
/// * `path` - Path to configuration file
/// * `schema` - Configuration schema for validation
///
/// # Returns
/// Loaded configuration or error with context
///
/// # Errors
/// Returns error if:
/// - File doesn't exist or can't be read
/// - TOML parsing fails
/// - Validation against schema fails
///
/// # Examples
/// ```no_run
/// let config = load_config("config.toml", &schema)?;
/// ```
pub fn load_config(path: &str, schema: &ConfigSchema) -> Result<Config> {
    // ...
}
```

## Licensing

### License Headers
All new files must include Apache 2.0 header:

```rust
// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

// ... rest of file
```

### Third-Party Code
- Only use compatible licenses (Apache 2.0, MIT, BSD)
- Add to NOTICE file
- Check with `cargo license`

## Issue Guidelines

### Reporting Issues
- Check if issue exists first
- Use clear, descriptive title
- Include reproduction steps
- Provide environment info: `rustc --version`
- Attach logs/errors if relevant

### Bug Report Template
```markdown
## Description
Brief description of the bug.

## Steps to Reproduce
1. Step 1
2. Step 2
3. Step 3

## Expected Behavior
What should happen?

## Actual Behavior
What actually happens?

## Environment
- Rust: `rustc --version`
- OS: (Linux/macOS/Windows)
- Execution Engine version: v0.5.0
- ABI version: v2.0.0
```

### Feature Request Template
```markdown
## Description
Brief description of desired feature.

## Motivation
Why is this feature needed?

## Proposed Solution
How should this work?

## Alternatives
Other approaches considered?
```

## Community

### Getting Help
- Read documentation in `docs/` directory
- Search existing issues
- Ask in Discussions tab
- Check SECURITY.md for security issues

### Staying Updated
- Watch this repository for updates
- Read CHANGELOG.md for changes
- Follow releases

## Recognition

Contributors will be:
- Listed in CONTRIBUTORS.md
- Credited in release notes
- Recognized in project statistics

## Project Structure

```
skylet/
├── abi/                    # Plugin ABI definitions (FFI)
├── src/                    # Core engine implementation
├── core/                   # Test framework and utilities
├── plugins/                # Built-in plugins
│   ├── logging/           # Logging service
│   ├── registry/          # Service registry
│   ├── config-manager/    # Configuration management
│   └── secrets-manager/   # Secret management
├── http-router/           # HTTP routing
├── job-queue/             # Background job queue
├── permissions/           # Permission system
├── plugin-packager/       # Plugin packaging utilities
├── docs/                  # Documentation
└── examples/              # Example plugins and code
```

## Good First Issues

New to the project? Look for issues labeled:
- `good first issue` - Great for newcomers
- `help wanted` - Extra attention needed
- `documentation` - Documentation improvements

## Communication Channels

| Channel | Purpose |
|---------|---------|
| [GitHub Discussions](https://github.com/vincents-ai/skylet/discussions) | Questions, ideas, show & tell |
| [GitHub Issues](https://github.com/vincents-ai/skylet/issues) | Bug reports, feature requests |
| 📧 Email: `shift+skylet@someone.section.me` | General inquiries |
| 🔒 Security: `shift+security@someone.section.me` | Security issues (private) |

## Questions?

- 📧 Email: `shift+skylet@someone.section.me`
- 💬 GitHub Discussions: [Link](https://github.com/vincents-ai/skylet/discussions)
- 🔒 Security: `shift+security@someone.section.me` (not public)

---

**Thank you for contributing to making Skylet better!** ❤️
