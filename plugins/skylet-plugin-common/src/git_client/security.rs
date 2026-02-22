//! Security validation for git operations
//!
//! This module provides input validation to prevent common security issues:
//! - **Path traversal** (CWE-22): Ensures paths stay within allowed directories
//! - **SSRF** (CWE-918): Validates URLs against allowed patterns
//! - **Command injection** (CWE-78): Sanitizes strings passed to git commands

use super::{GitError, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};

/// Security validator for git operations
///
/// This struct provides validation methods to prevent common security issues
/// in git operations.
#[derive(Debug, Clone)]
pub struct GitSecurityValidator {
    /// Base directories where git operations are allowed
    allowed_base_paths: Vec<PathBuf>,
    /// Allowed URL patterns for clone operations
    allowed_url_patterns: Vec<Regex>,
    /// Whether to allow local file:// URLs (default: false)
    allow_local_urls: bool,
}

impl Default for GitSecurityValidator {
    fn default() -> Self {
        Self {
            allowed_base_paths: vec![
                PathBuf::from("/tmp"),
                PathBuf::from("/var/tmp"),
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            ],
            allowed_url_patterns: vec![
                // HTTPS URLs to known git hosts
                Regex::new(r"^https://github\.com/[\w\-\.]+/[\w\-\.]+(\.git)?$").unwrap(),
                Regex::new(r"^https://gitlab\.com/[\w\-\.]+/[\w\-\.]+(\.git)?$").unwrap(),
                Regex::new(r"^https://bitbucket\.org/[\w\-\.]+/[\w\-\.]+(\.git)?$").unwrap(),
                Regex::new(r"^https://[\w\.\-]+\.github\.com/[\w\-\.]+/[\w\-\.]+(\.git)?$").unwrap(),
                Regex::new(r"^https://[\w\.\-]+\.gitlab\.com/[\w\-\.]+/[\w\-\.]+(\.git)?$").unwrap(),
                // SSH URLs (git@host:path format)
                Regex::new(r"^git@github\.com:[\w\-\.]+/[\w\-\.]+(\.git)?$").unwrap(),
                Regex::new(r"^git@gitlab\.com:[\w\-\.]+/[\w\-\.]+(\.git)?$").unwrap(),
                Regex::new(r"^git@bitbucket\.org:[\w\-\.]+/[\w\-\.]+(\.git)?$").unwrap(),
            ],
            allow_local_urls: false,
        }
    }
}

impl GitSecurityValidator {
    /// Create a new validator with custom configuration
    pub fn new(allowed_base_paths: Vec<PathBuf>, allow_local_urls: bool) -> Self {
        let default = Self::default();
        Self {
            allowed_base_paths: if allowed_base_paths.is_empty() {
                default.allowed_base_paths
            } else {
                allowed_base_paths
            },
            allowed_url_patterns: default.allowed_url_patterns,
            allow_local_urls,
        }
    }

    /// Add an allowed base path
    pub fn add_allowed_path(&mut self, path: PathBuf) {
        self.allowed_base_paths.push(path);
    }

    /// Add an allowed URL pattern
    pub fn add_allowed_url_pattern(&mut self, pattern: &str) -> Result<()> {
        Regex::new(pattern)
            .map(|regex| self.allowed_url_patterns.push(regex))
            .map_err(|e| GitError::configuration(format!("Invalid URL pattern '{}': {}", pattern, e)))
    }

    /// Validate that a path is within allowed directories
    ///
    /// # Security
    /// Prevents path traversal attacks by ensuring the resolved path
    /// is within one of the allowed base directories.
    pub fn validate_path(&self, path: &str) -> Result<PathBuf> {
        let path = PathBuf::from(path);
        
        // Check for suspicious patterns before canonicalization
        let path_str = path.to_string_lossy();
        if path_str.contains("..") {
            return Err(GitError::validation("Path contains '..' - potential path traversal"));
        }
        if path_str.contains('\0') {
            return Err(GitError::validation("Path contains null byte"));
        }

        // For paths that don't exist yet (like clone destinations), 
        // validate the parent directory exists and is allowed
        let (canonical_path, exists) = if path.exists() {
            (path.canonicalize().map_err(|e| {
                GitError::validation(format!("Cannot canonicalize path '{}': {}", path.display(), e))
            })?, true)
        } else {
            // Path doesn't exist - check parent
            let parent = path.parent().ok_or_else(|| {
                GitError::validation("Path has no parent directory")
            })?;
            
            if !parent.exists() {
                return Err(GitError::validation(format!(
                    "Parent directory does not exist: {}", parent.display()
                )));
            }
            
            let canonical_parent = parent.canonicalize().map_err(|e| {
                GitError::validation(format!("Cannot canonicalize parent '{}': {}", parent.display(), e))
            })?;
            
            // Reconstruct with canonical parent
            let file_name = path.file_name().ok_or_else(|| {
                GitError::validation("Path has no filename component")
            })?;
            (canonical_parent.join(file_name), false)
        };

        // Check if path is within allowed base paths
        let is_allowed = self.allowed_base_paths.iter().any(|base| {
            canonical_path.starts_with(base) || 
            (!exists && canonical_path.parent().map(|p| p.starts_with(base)).unwrap_or(false))
        });

        if !is_allowed {
            return Err(GitError::validation(format!(
                "Path '{}' is outside allowed directories: {}",
                canonical_path.display(),
                self.allowed_base_paths.iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        Ok(canonical_path)
    }

    /// Validate a git repository URL
    ///
    /// # Security
    /// Prevents SSRF by validating URLs against allowed patterns.
    /// Blocks file://, internal IP ranges, and unknown hosts.
    pub fn validate_url(&self, url: &str) -> Result<String> {
        // Check for null bytes
        if url.contains('\0') {
            return Err(GitError::validation("URL contains null byte"));
        }

        // Check for file:// URLs (local file access)
        if url.to_lowercase().starts_with("file://") {
            if !self.allow_local_urls {
                return Err(GitError::validation(
                    "file:// URLs are not allowed. Use allow_local_urls() to enable."
                ));
            }
            return Ok(url.to_string());
        }

        // Check for internal/private IP patterns in URL
        let blocked_patterns = [
            Regex::new(r"(?i)://localhost[:/]").unwrap(),
            Regex::new(r"(?i)://127\.\d+\.\d+\.\d+[:/]").unwrap(),
            Regex::new(r"(?i)://\[::1\][:/]").unwrap(),
            Regex::new(r"(?i)://10\.\d+\.\d+\.\d+[:/]").unwrap(),
            Regex::new(r"(?i)://172\.(1[6-9]|2\d|3[01])\.\d+\.\d+[:/]").unwrap(),
            Regex::new(r"(?i)://192\.168\.\d+\.\d+[:/]").unwrap(),
            Regex::new(r"(?i)://169\.254\.\d+\.\d+[:/]").unwrap(),
            Regex::new(r"(?i)://metadata\.").unwrap(),
            Regex::new(r"(?i)://169\.254\.169\.254[:/]").unwrap(),
        ];

        for pattern in &blocked_patterns {
            if pattern.is_match(url) {
                return Err(GitError::validation(
                    "URL points to internal/private address - potential SSRF"
                ));
            }
        }

        // Validate against allowed patterns
        let is_allowed = self.allowed_url_patterns.iter().any(|pattern| {
            pattern.is_match(url)
        });

        if !is_allowed {
            return Err(GitError::validation(format!(
                "URL '{}' does not match allowed patterns. \
                 Allowed hosts: github.com, gitlab.com, bitbucket.org (or add custom patterns)",
                url
            )));
        }

        Ok(url.to_string())
    }

    /// Sanitize a string for safe use in git command arguments
    ///
    /// # Security
    /// Checks for shell metacharacters to prevent command injection.
    /// Git commands receive arguments as separate argv items, but this
    /// provides defense-in-depth.
    pub fn sanitize_for_command(&self, input: &str) -> Result<String> {
        if input.contains('\0') {
            return Err(GitError::validation("Input contains null byte"));
        }

        let dangerous_patterns = [
            Regex::new(r"[;&|`$]").unwrap(),
            Regex::new(r"\$\([^)]*\)").unwrap(),
            Regex::new(r"`[^`]*`").unwrap(),
            Regex::new(r"[\r\n]").unwrap(),
            Regex::new(r"[<>]").unwrap(),
        ];

        for pattern in &dangerous_patterns {
            if pattern.is_match(input) {
                return Err(GitError::validation(format!(
                    "Input contains forbidden characters matching pattern: {}", 
                    pattern.as_str()
                )));
            }
        }

        if input.len() > 4096 {
            return Err(GitError::validation("Input exceeds maximum length (4096 bytes)"));
        }

        Ok(input.to_string())
    }

    /// Validate branch name
    pub fn validate_branch_name(&self, name: &str) -> Result<String> {
        if name.is_empty() {
            return Err(GitError::validation("Branch name cannot be empty"));
        }

        if name.contains('\0') || name.contains("..") {
            return Err(GitError::validation("Branch name contains forbidden characters"));
        }

        let valid_branch = Regex::new(r"^[a-zA-Z0-9_\-./]+$").unwrap();
        if !valid_branch.is_match(name) {
            return Err(GitError::validation(
                "Branch name contains invalid characters. \
                 Allowed: alphanumeric, underscore, hyphen, dot, forward slash"
            ));
        }

        self.sanitize_for_command(name)
    }

    /// Validate commit SHA
    pub fn validate_sha(&self, sha: &str) -> Result<String> {
        if sha.is_empty() {
            return Err(GitError::validation("SHA cannot be empty"));
        }

        let valid_sha = Regex::new(r"^[0-9a-fA-F]+$").unwrap();
        if !valid_sha.is_match(sha) {
            return Err(GitError::validation("SHA must be hexadecimal"));
        }

        if sha.len() < 7 || sha.len() > 40 {
            return Err(GitError::validation(
                "SHA must be between 7 and 40 characters"
            ));
        }

        Ok(sha.to_lowercase())
    }

    /// Validate file path (relative path within a repo)
    pub fn validate_file_path(&self, path: &str) -> Result<String> {
        if path.contains('\0') {
            return Err(GitError::validation("Path contains null byte"));
        }

        // Check for absolute path attempts
        if path.starts_with('/') {
            return Err(GitError::validation("Absolute paths not allowed"));
        }

        // Check for path traversal
        if path.contains("..") {
            return Err(GitError::validation("Path traversal not allowed"));
        }

        self.sanitize_for_command(path)
    }

    /// Validate commit message
    pub fn validate_commit_message(&self, message: &str) -> Result<String> {
        if message.is_empty() {
            return Err(GitError::validation("Commit message cannot be empty"));
        }

        if message.len() > 65536 {
            return Err(GitError::validation("Commit message exceeds maximum length"));
        }

        // Check for dangerous shell metacharacters
        // Note: We allow some characters that are common in commit messages
        let dangerous = Regex::new(r"[`$]").unwrap();
        if dangerous.is_match(message) {
            return Err(GitError::validation(
                "Commit message contains forbidden characters (backticks, $)"
            ));
        }

        Ok(message.to_string())
    }
}

/// Global security validator instance
static GIT_SECURITY: Lazy<GitSecurityValidator> = Lazy::new(GitSecurityValidator::default);

/// Get the global security validator
pub fn global_security() -> &'static GitSecurityValidator {
    &GIT_SECURITY
}

/// Validate a repository path using the global security validator
pub fn validate_repository_path(path: &str) -> Result<PathBuf> {
    global_security().validate_path(path)
}

/// Validate a repository URL using the global security validator
pub fn validate_repository_url(url: &str) -> Result<String> {
    global_security().validate_url(url)
}

/// Validate a branch name using the global security validator
pub fn validate_branch_name(name: &str) -> Result<String> {
    global_security().validate_branch_name(name)
}

/// Validate a commit SHA using the global security validator
pub fn validate_sha(sha: &str) -> Result<String> {
    global_security().validate_sha(sha)
}

/// Validate a file path using the global security validator
pub fn validate_file_path(path: &str) -> Result<String> {
    global_security().validate_file_path(path)
}

/// Validate a commit message using the global security validator
pub fn validate_commit_message(message: &str) -> Result<String> {
    global_security().validate_commit_message(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_traversal() {
        let validator = GitSecurityValidator::default();
        
        // Should reject path traversal
        assert!(validator.validate_path("/tmp/../etc/passwd").is_err());
        assert!(validator.validate_path("/tmp/../../../etc/passwd").is_err());
    }

    #[test]
    fn test_validate_path_allowed() {
        let validator = GitSecurityValidator::default();
        
        // Should allow paths in /tmp
        assert!(validator.validate_path("/tmp/test").is_ok());
    }

    #[test]
    fn test_validate_url_ssrf() {
        let validator = GitSecurityValidator::default();
        
        // Should reject internal IPs
        assert!(validator.validate_url("https://localhost/repo").is_err());
        assert!(validator.validate_url("https://127.0.0.1/repo").is_err());
        assert!(validator.validate_url("https://192.168.1.1/repo").is_err());
        assert!(validator.validate_url("https://10.0.0.1/repo").is_err());
        assert!(validator.validate_url("https://169.254.169.254/repo").is_err());
    }

    #[test]
    fn test_validate_url_allowed() {
        let validator = GitSecurityValidator::default();
        
        // Should allow known git hosts
        assert!(validator.validate_url("https://github.com/owner/repo").is_ok());
        assert!(validator.validate_url("https://gitlab.com/owner/repo").is_ok());
        assert!(validator.validate_url("https://bitbucket.org/owner/repo").is_ok());
        assert!(validator.validate_url("git@github.com:owner/repo.git").is_ok());
    }

    #[test]
    fn test_sanitize_command_injection() {
        let validator = GitSecurityValidator::default();
        
        // Should reject shell metacharacters
        assert!(validator.sanitize_for_command("test; rm -rf /").is_err());
        assert!(validator.sanitize_for_command("test && echo pwned").is_err());
        assert!(validator.sanitize_for_command("test | cat /etc/passwd").is_err());
        assert!(validator.sanitize_for_command("$(whoami)").is_err());
        assert!(validator.sanitize_for_command("`id`").is_err());
    }

    #[test]
    fn test_validate_branch_name() {
        let validator = GitSecurityValidator::default();
        
        // Valid branch names
        assert!(validator.validate_branch_name("main").is_ok());
        assert!(validator.validate_branch_name("feature/my-feature").is_ok());
        assert!(validator.validate_branch_name("fix-123").is_ok());
        
        // Invalid branch names
        assert!(validator.validate_branch_name("").is_err());
        assert!(validator.validate_branch_name("branch;rm -rf").is_err());
    }

    #[test]
    fn test_validate_sha() {
        let validator = GitSecurityValidator::default();
        
        // Valid SHAs
        assert!(validator.validate_sha("abc1234").is_ok());
        assert!(validator.validate_sha("0123456789abcdef0123456789abcdef01234567").is_ok());
        
        // Invalid SHAs
        assert!(validator.validate_sha("").is_err());
        assert!(validator.validate_sha("short").is_err());
        assert!(validator.validate_sha("ghijklm").is_err());
    }
}
