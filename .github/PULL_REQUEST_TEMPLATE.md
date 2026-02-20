## Description

<!-- Provide a brief description of your changes -->

## Type of Change

<!-- Mark the relevant option with [x] -->

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to change)
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Code refactoring (no functional changes)
- [ ] Security fix

## Component(s) Affected

<!-- Mark all that apply -->

- [ ] Core Runtime (`core/`)
- [ ] Plugin ABI (`abi/`)
- [ ] Permission System (`permissions/`)
- [ ] HTTP Router (`http-router/`)
- [ ] Job Queue (`job-queue/`)
- [ ] Plugin Packager (`plugin-packager/`)
- [ ] Documentation (`docs/`)
- [ ] CI/CD (`.github/`)
- [ ] Other: <!-- specify -->

## Checklist

<!-- Mark completed items with [x] -->

- [ ] I have read the [CONTRIBUTING](CONTRIBUTING.md) guidelines
- [ ] My code follows the project's code style
- [ ] I have added/updated tests as appropriate
- [ ] All new and existing tests pass locally
- [ ] I have updated documentation as needed
- [ ] I have checked for breaking ABI changes

## ABI Compatibility

<!-- If this PR affects the plugin ABI, describe the impact -->

- [ ] No ABI changes
- [ ] Backward-compatible ABI additions
- [ ] Breaking ABI changes (requires version bump)

## Security Considerations

<!-- Describe any security implications of this change -->

- [ ] No security implications
- [ ] Reviewed for memory safety
- [ ] Reviewed for permission/capability correctness
- [ ] Security-sensitive code has been audited

## Testing

<!-- Describe how you tested these changes -->

```bash
# Commands used to test
nix flake check
```

## Related Issues

<!-- Link related issues: Fixes #123, Closes #456 -->

## Additional Notes

<!-- Any additional context or screenshots -->
