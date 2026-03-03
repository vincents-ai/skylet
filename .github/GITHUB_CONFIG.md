# GitHub Configuration for Skylet

This document describes the GitHub organization and repository configuration for the Skylet project.

## Repository Settings

### Branch Protection (main)

The `main` branch is protected with:
- ✅ Require pull request reviews (1 approval minimum)
- ✅ Require status checks to pass
- ✅ Require branches to be up to date
- ✅ Require signed commits
- ✅ Restrict force pushes
- ✅ Restrict deletions

### Required Status Checks

All PRs must pass:
- ✅ Tests (Linux, macOS, Windows)
- ✅ Code formatting (rustfmt)
- ✅ Linting (clippy)
- ✅ Documentation generation
- ✅ Security audit

## Discussion Categories

The project uses GitHub Discussions for community engagement:

| Category | Purpose | Use For |
|----------|---------|---------|
| **Announcements** | Official updates | Releases, security notices, breaking changes |
| **General** | Open discussion | Architecture, best practices, feedback |
| **Ideas** | Feature proposals | RFC discussions, design proposals |
| **Q&A** | Questions | How-to, troubleshooting, configuration help |
| **Show & Tell** | Community showcase | Projects, plugins, success stories |

## Issue Labels

### Type Labels
| Label | Color | Description |
|-------|-------|-------------|
| `bug` | `#d73a4a` | Something isn't working |
| `enhancement` | `#a2eeef` | New feature or improvement |
| `documentation` | `#0075ca` | Documentation improvements |
| `security` | `#0e8a16` | Security vulnerability or fix |
| `performance` | `#fbca04` | Performance improvement |
| `refactoring` | `#1d76db` | Code quality improvement |

### Priority Labels
| Label | Color | Description |
|-------|-------|-------------|
| `priority: critical` | `#b60205` | Must fix immediately |
| `priority: high` | `#d93f0b` | Important |
| `priority: medium` | `#fbca04` | Normal priority |
| `priority: low` | `#0e8a16` | Nice to have |

### Status Labels
| Label | Color | Description |
|-------|-------|-------------|
| `help wanted` | `#008672` | Extra attention needed |
| `good first issue` | `#7057ff` | Good for newcomers |
| `in progress` | `#fbca04` | Currently being worked on |
| `blocked` | `#b60205` | Blocked by external factor |
| `stale` | `#eeeeee` | No recent activity |

### Component Labels
| Label | Color | Description |
|-------|-------|-------------|
| `component: core` | `#1d76db` | Core runtime |
| `component: abi` | `#1d76db` | Plugin ABI |
| `component: plugins` | `#1d76db` | Built-in plugins |
| `component: docs` | `#1d76db` | Documentation |
| `component: ci` | `#1d76db` | CI/CD pipelines |

### Resolution Labels
| Label | Color | Description |
|-------|-------|-------------|
| `wontfix` | `#ffffff` | Will not be fixed |
| `duplicate` | `#cfd3d7` | Duplicate issue |
| `invalid` | `#e4e669` | Invalid issue |

## Milestone Tracking

| Milestone | Status | Description |
|-----------|--------|-------------|
| `v0.5.x` | Current | Beta release patches |
| `v0.6.0` | Next | Next minor release |
| `v1.0.0` | Planned | Stable release |
| `v2.0.0` | Future | Major release with ABI changes |

## Teams

| Team | Responsibility |
|------|---------------|
| `@vincents-ai/core-team` | Core engine development |
| `@vincents-ai/security` | Security reviews |
| `@vincents-ai/docs` | Documentation |
| `@vincents-ai/plugins` | Plugin ecosystem |
| `@vincents-ai/networking` | HTTP router, networking |

## Automated Workflows

### CI/CD Workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `test.yml` | Push, PR | Run test suite |
| `security.yml` | Push, PR, Schedule | Security audit |
| `docs.yml` | Push to main | Deploy documentation |
| `release.yml` | Tag push | Create release |

### Automation Features

- **Dependabot**: Weekly dependency updates (Mondays 06:00 UTC)
- **Stale Bot**: Closes inactive issues/PRs after 60 days
- **Release Automation**: Automatic changelog, binaries, and crate publishing

## Code Owners

See [.github/CODEOWNERS](CODEOWNERS) for automatic review assignments.

## Security

See [SECURITY.md](../SECURITY.md) for security policies.

- **Security issues**: Email `shift+security@someone.section.me`
- **Non-critical**: Use the security issue template

## External Integrations

| Service | Purpose |
|---------|---------|
| GitHub Pages | Documentation hosting |
| crates.io | Rust package registry |
| GitHub Releases | Binary distribution |

## Configuration Files

| File | Purpose |
|------|---------|
| `CODEOWNERS` | Automatic review assignments |
| `FUNDING.yml` | Sponsorship information |
| `dependabot.yml` | Dependency automation |
| `stale.yml` | Stale issue management |
| `ISSUE_TEMPLATE/` | Issue templates |
| `DISCUSSION_TEMPLATE/` | Discussion templates |
| `workflows/` | GitHub Actions workflows |
