# GitHub Configuration for Execution Engine

## Discussion Categories

The project uses GitHub Discussions for community engagement:

### 1. **Announcements**
- New releases and updates
- Important security notices
- Breaking changes

### 2. **General Discussion**
- Questions about usage
- Best practices
- Architecture discussions

### 3. **Show & Tell**
- Projects using Execution Engine
- Plugins developed by community
- Success stories

### 4. **Help & Support**
- How-to questions
- Troubleshooting
- Configuration help

### 5. **Ideas**
- Feature requests
- Design proposals
- RFC discussions

## Issue Labels

- **bug** - Something isn't working
- **enhancement** - New feature or improvement
- **documentation** - Improvements or additions to documentation
- **security** - Security vulnerability or fix
- **help wanted** - Extra attention is needed
- **good first issue** - Good for newcomers
- **question** - Further information is requested
- **wontfix** - This will not be worked on
- **performance** - Performance improvement needed
- **refactoring** - Code quality improvement

## Milestone Tracking

- **v2.0.x** - Current stable release
- **v2.1.0** - Next minor release
- **v3.0.0** - Future major release

## Branch Protection

Main branch is protected with:
- Require pull request reviews
- Require status checks to pass
- Require branches to be up to date
- Restrict who can push to matching branches

## Automated Checks

All PRs must pass:
- ✅ Tests (all platforms)
- ✅ Code formatting (rustfmt)
- ✅ Linting (clippy)
- ✅ Documentation generation
- ✅ Security audit
