# Security Policy

## Supported Versions

The following versions of the execution engine are currently supported with security updates:

| Version | Supported          |
| ------- | ------------------ |
| 0.2.x   | :white_check_mark: |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability, please report it responsibly.

### How to Report

1. **DO NOT** create a public GitHub issue for security vulnerabilities
2. Email the security team at: [shift+security@someone.section.me](mailto:shift+security@someone.section.me)
3. Include in your report:
   - Description of the vulnerability
   - Steps to reproduce the issue
   - Potential impact assessment
   - Any suggested fixes (optional)

### What to Expect

- Acknowledgment of your report within 48 hours
- Regular updates on the progress of fixing the vulnerability
- Credit in the security advisory (if desired) once the issue is resolved

## Security Best Practices

When developing plugins for the execution engine:

- **Never hardcode secrets** in source code - use the secrets-manager plugin
- **Validate all inputs** from untrusted sources
- **Use the plugin ABI** for secure communication between plugins
- **Follow the principle of least privilege** when requesting capabilities

## Security Features

The execution engine includes several security features:

- **Plugin sandboxing**: Plugins run in isolated contexts
- **Secrets encryption**: AES-256-GCM encryption for sensitive data
- **Capability-based access**: Plugins declare required capabilities at load time
- **Digital signatures**: Ed25519 signatures for plugin verification

## Related Documents

- [Security Architecture Documentation](docs/security.md)
- [Plugin ABI Security Considerations](abi/README.md)
- [Secrets Management Guide](plugins/secrets-manager/README.md)
