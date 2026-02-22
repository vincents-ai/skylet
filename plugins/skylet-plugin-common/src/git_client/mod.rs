//! Git operations client for plugin automation
//!
//! **Status: DISABLED - Requires Architectural Refactoring**
//!
//! This module provides a unified interface for Git operations supporting multiple providers
//! (GitHub, GitLab, Bitbucket) and local Git operations.
//!
//! ## Security
//!
//! This module includes comprehensive input validation to prevent:
//! - **Path traversal** (CWE-22): Paths validated to stay within allowed directories
//! - **SSRF** (CWE-918): URLs validated against allowed patterns
//! - **Command injection** (CWE-78): Input sanitized before use in git commands
//!
//! Use the `security` module for validation:
//! ```rust,ignore
//! use skylet_plugin_common::git_client::security::{
//!     validate_repository_path, validate_repository_url, validate_branch_name
//! };
//! 
//! // Validate before use
//! let safe_path = validate_repository_path("/tmp/myrepo")?;
//! let safe_url = validate_repository_url("https://github.com/owner/repo")?;
//! let safe_branch = validate_branch_name("feature/my-feature")?;
//! ```

pub mod adapters;
pub mod security;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Error Types
// ============================================================================

/// Error type for Git operations
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Authentication failed: {0}")]
    Authentication(String),
    
    #[error("Clone failed: {0}")]
    CloneFailed(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("API error: {0}")]
    Api(String),
    
    #[error("Branch not found: {0}")]
    BranchNotFound(String),
    
    #[error("Tag not found: {0}")]
    TagNotFound(String),
    
    #[error("File not found: {0}")]
    FileNotFound(String),
    
    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),
    
    #[error("Merge conflict: {0}")]
    MergeConflict(String),
    
    #[error("HTTP error: {0}")]
    Http(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl GitError {
    pub fn authentication(msg: impl Into<String>) -> Self {
        GitError::Authentication(msg.into())
    }
    
    pub fn clone_failed(msg: impl Into<String>) -> Self {
        GitError::CloneFailed(msg.into())
    }
    
    pub fn configuration(msg: impl Into<String>) -> Self {
        GitError::Configuration(msg.into())
    }
    
    pub fn api(msg: impl Into<String>) -> Self {
        GitError::Api(msg.into())
    }
    
    pub fn branch_not_found(branch: impl Into<String>) -> Self {
        GitError::BranchNotFound(branch.into())
    }
    
    pub fn tag_not_found(tag: impl Into<String>) -> Self {
        GitError::TagNotFound(tag.into())
    }
    
    pub fn file_not_found(path: impl Into<String>) -> Self {
        GitError::FileNotFound(path.into())
    }
    
    pub fn repository_not_found(repo: impl Into<String>) -> Self {
        GitError::RepositoryNotFound(repo.into())
    }
    
    pub fn merge_conflict(msg: impl Into<String>) -> Self {
        GitError::MergeConflict(msg.into())
    }
    
    pub fn http(msg: impl Into<String>) -> Self {
        GitError::Http(msg.into())
    }
    
    pub fn validation(msg: impl Into<String>) -> Self {
        GitError::Validation(msg.into())
    }
    
    pub fn unknown(msg: impl Into<String>) -> Self {
        GitError::Unknown(msg.into())
    }
}

/// Convert ureq errors to GitError
impl From<ureq::Error> for GitError {
    fn from(err: ureq::Error) -> Self {
        GitError::Http(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, GitError>;

// ============================================================================
// Configuration Types
// ============================================================================

/// Git client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub provider: GitProvider,
    pub token: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub base_url: Option<String>,
    pub timeout_seconds: u64,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            provider: GitProvider::GitHub,
            token: None,
            username: None,
            password: None,
            base_url: None,
            timeout_seconds: 30,
        }
    }
}

/// Supported Git providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitProvider {
    GitHub,
    GitLab,
    Bitbucket,
    Local,
}

// ============================================================================
// User Types
// ============================================================================

/// Git user information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitUser {
    pub id: u64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub html_url: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub site_admin: bool,
}

// ============================================================================
// Repository Types
// ============================================================================

/// Repository information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepository {
    pub name: String,
    pub url: String,
    pub default_branch: String,
    pub description: Option<String>,
    pub is_private: bool,
    pub is_fork: bool,
    pub language: Option<String>,
    pub stargazers_count: u32,
    pub watchers_count: u32,
    pub forks_count: u32,
    pub open_issues_count: u32,
    pub size: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub pushed_at: DateTime<Utc>,
    pub owner: GitUser,
}

// ============================================================================
// Commit Types
// ============================================================================

/// Commit statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStats {
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
}

/// Tree reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitTree {
    pub sha: String,
    pub url: String,
}

/// Commit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommit {
    pub sha: String,
    pub message: String,
    pub author: GitUser,
    pub committer: GitUser,
    pub tree: GitTree,
    pub parents: Vec<String>,
    pub stats: Option<GitStats>,
    pub url: String,
    pub html_url: String,
    pub timestamp: DateTime<Utc>,
    pub added_files: Vec<String>,
    pub modified_files: Vec<String>,
    pub deleted_files: Vec<String>,
}

// ============================================================================
// Branch Types
// ============================================================================

/// Branch information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBranch {
    pub name: String,
    pub commit: GitCommit,
    pub protected: bool,
    pub default: bool,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub upstream: Option<String>,
}

// ============================================================================
// Tag Types
// ============================================================================

/// Tag information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitTag {
    pub name: String,
    pub commit: GitCommit,
    pub tagger: Option<GitUser>,
    pub message: Option<String>,
    pub zipball_url: String,
    pub tarball_url: String,
    pub timestamp: DateTime<Utc>,
}

// ============================================================================
// Clone Types
// ============================================================================

/// Clone operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCloneResult {
    pub repository_url: String,
    pub clone_path: String,
    pub cloned_to: String,
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
    pub clone_time_ms: u64,
}

// ============================================================================
// File Types
// ============================================================================

/// File information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFile {
    pub path: String,
    pub name: String,
    pub sha: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub type_: String,
    pub encoding: Option<String>,
    pub content: Option<String>,
    pub download_url: String,
    pub html_url: String,
    pub git_url: String,
}

/// File change operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitFileOperation {
    Create,
    Update,
    Delete,
}

/// File change for commit operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFileChange {
    pub path: String,
    pub operation: GitFileOperation,
    pub content: String,
}

// ============================================================================
// Diff Types
// ============================================================================

/// Diff information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiff {
    pub file_path: String,
    pub a_mode: String,
    pub b_mode: String,
    pub a_sha: Option<String>,
    pub b_sha: Option<String>,
    pub a_path: Option<String>,
    pub b_path: Option<String>,
    pub diff: String,
    pub new_file: bool,
    pub deleted_file: bool,
    pub renamed_file: bool,
}

// ============================================================================
// Status Types
// ============================================================================

/// Repository status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub modified: Vec<String>,
    pub added: Vec<String>,
    pub deleted: Vec<String>,
    pub renamed: Vec<(String, String)>,
    pub untracked: Vec<String>,
    pub branch: String,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub clean: bool,
}

// ============================================================================
// Merge Types
// ============================================================================

/// Merge options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitMergeOptions {
    pub no_ff: bool,
    pub squash: bool,
    pub message: Option<String>,
}

impl Default for GitMergeOptions {
    fn default() -> Self {
        Self {
            no_ff: false,
            squash: false,
            message: None,
        }
    }
}

/// Merge result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitMergeResult {
    pub success: bool,
    pub merged_commit: Option<String>,
    pub conflicts: Vec<String>,
    pub error_message: Option<String>,
    pub merge_time_ms: u64,
}

// ============================================================================
// Pull Request Types
// ============================================================================

/// Pull request state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitPullRequestState {
    Open,
    Closed,
    Merged,
}

/// Pull request branch reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitPullRequestBranch {
    pub label: String,
    #[serde(rename = "ref")]
    pub ref_: String,
    pub sha: String,
    pub repo: GitRepository,
}

/// Pull request information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitPullRequest {
    pub id: u64,
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    pub state: GitPullRequestState,
    pub user: GitUser,
    pub assignee: Option<GitUser>,
    pub head: GitPullRequestBranch,
    pub base: GitPullRequestBranch,
    pub merged: bool,
    pub mergeable: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub html_url: String,
    pub diff_url: String,
    pub patch_url: String,
    pub review_comments: u32,
    pub commits: u32,
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
}

// ============================================================================
// Workflow Types
// ============================================================================

/// Workflow state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitWorkflowState {
    Active,
    Inactive,
    Deleted,
}

/// Workflow status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitWorkflowStatus {
    Queued,
    InProgress,
    Completed,
}

/// Workflow conclusion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitWorkflowConclusion {
    Success,
    Failure,
    Neutral,
    Cancelled,
    Skipped,
    TimedOut,
    ActionRequired,
}

/// Workflow information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitWorkflow {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub state: GitWorkflowState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub html_url: String,
    pub badge_url: String,
}

/// Workflow run information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitWorkflowRun {
    pub id: u64,
    pub workflow: String,
    pub name: Option<String>,
    pub head_branch: String,
    pub head_sha: String,
    pub status: GitWorkflowStatus,
    pub conclusion: Option<GitWorkflowConclusion>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub url: String,
    pub html_url: String,
    pub jobs_url: String,
    pub logs_url: String,
    pub check_suite_url: String,
    pub run_number: u64,
    pub event: String,
}

// ============================================================================
// Target Types (for unified API)
// ============================================================================

/// Unified target for Git operations (remote or local)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GitTarget {
    /// Remote repository identified by owner and name
    Remote { owner: String, name: String },
    /// Local repository at filesystem path
    Local { path: String },
}

// ============================================================================
// Trait Definition (Remote API)
// ============================================================================

/// Git client trait for remote Git operations (GitHub, GitLab, etc.)
/// 
/// Use this trait for implementations that communicate with remote Git APIs.
/// For local filesystem operations, see `LocalGitClient`.
#[async_trait]
pub trait GitClient: Send + Sync {
    /// Initialize the client with configuration
    async fn initialize(&mut self, config: GitConfig) -> Result<()>;
    
    /// Test connection to the Git provider
    async fn test_connection(&self) -> Result<bool>;
    
    /// Clone a repository
    async fn clone_repository(&self, repository_url: &str, destination: &str) -> Result<GitCloneResult>;
    
    /// Get repository information
    async fn get_repository(&self, owner: &str, name: &str) -> Result<Option<GitRepository>>;
    
    /// List repositories
    async fn list_repositories(&self, owner: Option<&str>, organization: Option<&str>) -> Result<Vec<GitRepository>>;
    
    /// Get a specific commit
    async fn get_commit(&self, owner: &str, name: &str, sha: &str) -> Result<Option<GitCommit>>;
    
    /// List commits in a repository
    async fn list_commits(&self, owner: &str, name: &str, branch: Option<&str>, limit: Option<u32>) -> Result<Vec<GitCommit>>;
    
    /// Create a commit with file changes (local operation)
    async fn create_commit(&self, repository_path: &str, message: &str, files: Vec<GitFileChange>) -> Result<GitCommit>;
    
    /// List branches in a repository
    async fn list_branches(&self, owner: &str, name: &str) -> Result<Vec<GitBranch>>;
    
    /// Create a new branch (local operation)
    async fn create_branch(&self, repository_path: &str, branch_name: &str, from_branch: Option<&str>) -> Result<GitBranch>;
    
    /// Delete a branch (local operation)
    async fn delete_branch(&self, repository_path: &str, branch_name: &str) -> Result<()>;
    
    /// Merge branches (local operation)
    async fn merge_branch(&self, repository_path: &str, source_branch: &str, target_branch: &str, options: GitMergeOptions) -> Result<GitMergeResult>;
    
    /// List tags in a repository
    async fn list_tags(&self, owner: &str, name: &str) -> Result<Vec<GitTag>>;
    
    /// Create a tag (local operation)
    async fn create_tag(&self, repository_path: &str, tag_name: &str, target: &str, message: &str) -> Result<GitTag>;
    
    /// Delete a tag (local operation)
    async fn delete_tag(&self, repository_path: &str, tag_name: &str) -> Result<()>;
    
    /// List pull requests
    async fn list_pull_requests(&self, owner: &str, name: &str, state: Option<GitPullRequestState>) -> Result<Vec<GitPullRequest>>;
    
    /// Create a pull request
    async fn create_pull_request(&self, owner: &str, name: &str, title: &str, body: &str, head: &str, base: &str) -> Result<GitPullRequest>;
    
    /// List workflows in a repository
    async fn list_workflows(&self, owner: &str, name: &str) -> Result<Vec<GitWorkflow>>;
    
    /// Trigger a workflow
    async fn trigger_workflow(&self, owner: &str, name: &str, workflow: &str, inputs: HashMap<String, String>) -> Result<GitWorkflowRun>;
    
    /// List workflow runs
    async fn list_workflow_runs(&self, owner: &str, name: &str, workflow: Option<&str>) -> Result<Vec<GitWorkflowRun>>;
    
    /// Get file contents
    async fn get_file(&self, owner: &str, name: &str, path: &str, branch: Option<&str>) -> Result<Option<GitFile>>;
    
    /// Create a file (local operation)
    async fn create_file(&self, repository_path: &str, path: &str, content: &str, message: &str) -> Result<GitFile>;
    
    /// Delete a file (local operation)
    async fn delete_file(&self, repository_path: &str, path: &str, message: &str) -> Result<()>;
    
    /// Get diff between refs (local operation)
    async fn get_diff(&self, repository_path: &str, from: &str, to: &str) -> Result<Vec<GitDiff>>;
    
    /// Get repository status (local operation)
    async fn get_status(&self, repository_path: &str) -> Result<GitStatus>;
    
    /// Get commit from SHA (local operation)
    async fn get_commit_from_sha(&self, repository_path: &str, sha: &str) -> Result<GitCommit>;
    
    /// Get branch from name (local operation)
    async fn get_branch_from_name(&self, repository_path: &str, branch_name: &str) -> Result<GitBranch>;
    
    /// Get tag from name (local operation)
    async fn get_tag_from_name(&self, repository_path: &str, tag_name: &str) -> Result<GitTag>;
    
    /// Get file from path (local operation)
    async fn get_file_from_path(&self, repository_path: &str, path: &str) -> Result<GitFile>;
}
