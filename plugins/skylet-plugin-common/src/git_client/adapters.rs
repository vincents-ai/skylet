// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// Git client adapters for different Git implementations
// Provides implementations for GitHub, GitLab, Bitbucket, and local Git operations
use super::*;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::process::Command;
use std::fs;
use std::path::Path;

// GitHub client
pub struct GitHubClient {
    config: Option<GitConfig>,
    token: Option<String>,
    base_url: String,
}

impl GitHubClient {
    pub fn new() -> Self {
        Self {
            config: None,
            token: None,
            base_url: "https://api.github.com".to_string(),
        }
    }

    fn build_request(&self, path: &str) -> Result<ureq::Request, GitError> {
        let agent = ureq::AgentBuilder::new().build();
        let url = format!("{}{}", self.base_url, path);
        let mut request = agent.get(&url);
        
        if let Some(ref token) = self.token {
            request = request.set("Authorization", &format!("token {}", token));
        }
        
        Ok(request)
    }

    fn build_post_request(&self, path: &str) -> Result<ureq::Request, GitError> {
        let agent = ureq::AgentBuilder::new().build();
        let url = format!("{}{}", self.base_url, path);
        let mut request = agent.post(&url);
        
        if let Some(ref token) = self.token {
            request = request.set("Authorization", &format!("token {}", token));
        }
        
        Ok(request)
    }

    fn build_put_request(&self, path: &str) -> Result<ureq::Request, GitError> {
        let agent = ureq::AgentBuilder::new().build();
        let url = format!("{}{}", self.base_url, path);
        let mut request = agent.put(&url);
        
        if let Some(ref token) = self.token {
            request = request.set("Authorization", &format!("token {}", token));
        }
        
        Ok(request)
    }

    fn build_delete_request(&self, path: &str) -> Result<ureq::Request, GitError> {
        let agent = ureq::AgentBuilder::new().build();
        let url = format!("{}{}", self.base_url, path);
        let mut request = agent.delete(&url);
        
        if let Some(ref token) = self.token {
            request = request.set("Authorization", &format!("token {}", token));
        }
        
        Ok(request)
    }
}

#[async_trait]
impl GitClient for GitHubClient {
    async fn initialize(&mut self, config: GitConfig) -> Result<()> {
        self.config = Some(config.clone());
        
        // Extract token from config
        if let Some(ref token) = config.token {
            self.token = Some(token.clone());
        } else if let (Some(ref username), Some(ref password)) = (&config.username, &config.password) {
            // GitHub doesn't support username/password for API, would need to get token
            return Err(GitError::authentication("GitHub API requires token authentication"));
        }
        
        Ok(())
    }

    async fn test_connection(&self) -> Result<bool> {
        let request = self.build_request("/user")?;
        match request.call() {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn clone_repository(&self, repository_url: &str, destination: &str) -> Result<GitCloneResult> {
        let start_time = std::time::Instant::now();
        
        // Use git command to clone
        let mut cmd = Command::new("git");
        cmd.arg("clone").arg(repository_url).arg(destination);
        
        let output = cmd.output().map_err(|e| GitError::clone_failed(e.to_string()))?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Ok(GitCloneResult {
                repository_url: repository_url.to_string(),
                clone_path: destination.to_string(),
                cloned_to: destination.to_string(),
                commit: None,
                branch: None,
                success: false,
                error_message: Some(error.to_string()),
                clone_time_ms: start_time.elapsed().as_millis() as u64,
            });
        }
        
        // Get current commit and branch
        let commit = self.get_current_commit(destination)?;
        let branch = self.get_current_branch(destination)?;
        
        Ok(GitCloneResult {
            repository_url: repository_url.to_string(),
            clone_path: destination.to_string(),
            cloned_to: destination.to_string(),
            commit,
            branch,
            success: true,
            error_message: None,
            clone_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    async fn get_repository(&self, owner: &str, name: &str) -> Result<Option<GitRepository>> {
        let request = self.build_request(&format!("/repos/{}/{}", owner, name))?;
        let response = request.call()?;
        
        if response.status() == 404 {
            return Ok(None);
        }
        
        let repo: serde_json::Value = response.into_json()?;
        
        Ok(Some(GitRepository {
            name: repo["name"].as_str().unwrap_or("").to_string(),
            url: repo["html_url"].as_str().unwrap_or("").to_string(),
            default_branch: repo["default_branch"].as_str().unwrap_or("main").to_string(),
            description: repo["description"].as_str().map(|s| s.to_string()),
            is_private: repo["private"].as_bool().unwrap_or(false),
            is_fork: repo["fork"].as_bool().unwrap_or(false),
            language: repo["language"].as_str().map(|s| s.to_string()),
            stargazers_count: repo["stargazers_count"].as_u64().unwrap_or(0) as u32,
            watchers_count: repo["watchers_count"].as_u64().unwrap_or(0) as u32,
            forks_count: repo["forks_count"].as_u64().unwrap_or(0) as u32,
            open_issues_count: repo["open_issues_count"].as_u64().unwrap_or(0) as u32,
            size: repo["size"].as_u64().unwrap_or(0),
            created_at: DateTime::parse_from_rfc3339(repo["created_at"].as_str().unwrap_or(""))
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(repo["updated_at"].as_str().unwrap_or(""))
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            pushed_at: DateTime::parse_from_rfc3339(repo["pushed_at"].as_str().unwrap_or(""))
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            owner: GitUser {
                id: repo["owner"]["id"].as_u64().unwrap_or(0),
                login: repo["owner"]["login"].as_str().unwrap_or("").to_string(),
                name: repo["owner"]["name"].as_str().map(|s| s.to_string()),
                email: repo["owner"]["email"].as_str().map(|s| s.to_string()),
                avatar_url: repo["owner"]["avatar_url"].as_str().map(|s| s.to_string()),
                html_url: repo["owner"]["html_url"].as_str().map(|s| s.to_string()),
                type_: repo["owner"]["type"].as_str().map(|s| s.to_string()),
                site_admin: repo["owner"]["site_admin"].as_bool().unwrap_or(false),
            },
        }))
    }

    async fn list_repositories(&self, owner: Option<&str>, organization: Option<&str>) -> Result<Vec<GitRepository>> {
        let path = if let Some(org) = organization {
            format!("/orgs/{}/repos", org)
        } else if let Some(user) = owner {
            format!("/users/{}/repos", user)
        } else {
            return Err(GitError::configuration("Either owner or organization must be specified"));
        };
        
        let request = self.build_request(&path)?;
        let response = request.call()?;
        let repos: serde_json::Value = response.into_json()?;
        
        let repositories: Vec<GitRepository> = repos.as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|repo| GitRepository {
                name: repo["name"].as_str().unwrap_or("").to_string(),
                url: repo["html_url"].as_str().unwrap_or("").to_string(),
                default_branch: repo["default_branch"].as_str().unwrap_or("main").to_string(),
                description: repo["description"].as_str().map(|s| s.to_string()),
                is_private: repo["private"].as_bool().unwrap_or(false),
                is_fork: repo["fork"].as_bool().unwrap_or(false),
                language: repo["language"].as_str().map(|s| s.to_string()),
                stargazers_count: repo["stargazers_count"].as_u64().unwrap_or(0) as u32,
                watchers_count: repo["watchers_count"].as_u64().unwrap_or(0) as u32,
                forks_count: repo["forks_count"].as_u64().unwrap_or(0) as u32,
                open_issues_count: repo["open_issues_count"].as_u64().unwrap_or(0) as u32,
                size: repo["size"].as_u64().unwrap_or(0),
                created_at: DateTime::parse_from_rfc3339(repo["created_at"].as_str().unwrap_or(""))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(repo["updated_at"].as_str().unwrap_or(""))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                pushed_at: DateTime::parse_from_rfc3339(repo["pushed_at"].as_str().unwrap_or(""))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                owner: GitUser {
                    id: repo["owner"]["id"].as_u64().unwrap_or(0),
                    login: repo["owner"]["login"].as_str().unwrap_or("").to_string(),
                    name: repo["owner"]["name"].as_str().map(|s| s.to_string()),
                    email: repo["owner"]["email"].as_str().map(|s| s.to_string()),
                    avatar_url: repo["owner"]["avatar_url"].as_str().map(|s| s.to_string()),
                    html_url: repo["owner"]["html_url"].as_str().map(|s| s.to_string()),
                    type_: repo["owner"]["type"].as_str().map(|s| s.to_string()),
                    site_admin: repo["owner"]["site_admin"].as_bool().unwrap_or(false),
                },
            })
            .collect();
        
        Ok(repositories)
    }

    async fn get_commit(&self, owner: &str, name: &str, sha: &str) -> Result<Option<GitCommit>> {
        let request = self.build_request(&format!("/repos/{}/{}/commits/{}", owner, name, sha))?;
        let response = request.call()?;
        
        if response.status() == 404 {
            return Ok(None);
        }
        
        let commit: serde_json::Value = response.into_json()?;
        
        Ok(Some(GitCommit {
            sha: commit["sha"].as_str().unwrap_or("").to_string(),
            message: commit["commit"]["message"].as_str().unwrap_or("").to_string(),
            author: GitUser {
                id: commit["author"]["id"].as_u64().unwrap_or(0),
                login: commit["author"]["login"].as_str().unwrap_or("").to_string(),
                name: commit["author"]["name"].as_str().map(|s| s.to_string()),
                email: commit["author"]["email"].as_str().map(|s| s.to_string()),
                avatar_url: commit["author"]["avatar_url"].as_str().map(|s| s.to_string()),
                html_url: commit["author"]["html_url"].as_str().map(|s| s.to_string()),
                type_: commit["author"]["type"].as_str().map(|s| s.to_string()),
                site_admin: commit["author"]["site_admin"].as_bool().unwrap_or(false),
            },
            committer: GitUser {
                id: commit["committer"]["id"].as_u64().unwrap_or(0),
                login: commit["committer"]["login"].as_str().unwrap_or("").to_string(),
                name: commit["committer"]["name"].as_str().map(|s| s.to_string()),
                email: commit["committer"]["email"].as_str().map(|s| s.to_string()),
                avatar_url: commit["committer"]["avatar_url"].as_str().map(|s| s.to_string()),
                html_url: commit["committer"]["html_url"].as_str().map(|s| s.to_string()),
                type_: commit["committer"]["type"].as_str().map(|s| s.to_string()),
                site_admin: commit["committer"]["site_admin"].as_bool().unwrap_or(false),
            },
            tree: GitTree {
                sha: commit["tree"]["sha"].as_str().unwrap_or("").to_string(),
                url: commit["tree"]["url"].as_str().unwrap_or("").to_string(),
            },
            parents: commit["parents"].as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|p| p["sha"].as_str().unwrap_or("").to_string())
                .collect(),
            stats: commit["stats"].as_object().map(|stats| GitStats {
                additions: stats["additions"].as_u64().unwrap_or(0) as u32,
                deletions: stats["deletions"].as_u64().unwrap_or(0) as u32,
                changed_files: stats["changed_files"].as_u64().unwrap_or(0) as u32,
            }),
            url: commit["url"].as_str().unwrap_or("").to_string(),
            html_url: commit["html_url"].as_str().unwrap_or("").to_string(),
            timestamp: DateTime::parse_from_rfc3339(commit["commit"]["author"]["date"].as_str().unwrap_or(""))
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            added_files: vec![],
            modified_files: vec![],
            deleted_files: vec![],
        }))
    }

    async fn list_commits(&self, owner: &str, name: &str, branch: Option<&str>, limit: Option<u32>) -> Result<Vec<GitCommit>> {
        let path = format!("/repos/{}/{}/commits", owner, name);
        let mut request = self.build_request(&path)?;
        
        if let Some(branch) = branch {
            request = request.set("sha", branch);
        }
        
        if let Some(limit) = limit {
            request = request.set("per_page", &limit.to_string());
        }
        
        let response = request.call()?;
        let commits: serde_json::Value = response.into_json()?;
        
        let commit_list: Vec<GitCommit> = commits.as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|commit| GitCommit {
                sha: commit["sha"].as_str().unwrap_or("").to_string(),
                message: commit["commit"]["message"].as_str().unwrap_or("").to_string(),
                author: GitUser {
                    id: commit["author"]["id"].as_u64().unwrap_or(0),
                    login: commit["author"]["login"].as_str().unwrap_or("").to_string(),
                    name: commit["author"]["name"].as_str().map(|s| s.to_string()),
                    email: commit["author"]["email"].as_str().map(|s| s.to_string()),
                    avatar_url: commit["author"]["avatar_url"].as_str().map(|s| s.to_string()),
                    html_url: commit["author"]["html_url"].as_str().map(|s| s.to_string()),
                    type_: commit["author"]["type"].as_str().map(|s| s.to_string()),
                    site_admin: commit["author"]["site_admin"].as_bool().unwrap_or(false),
                },
                committer: GitUser {
                    id: commit["committer"]["id"].as_u64().unwrap_or(0),
                    login: commit["committer"]["login"].as_str().unwrap_or("").to_string(),
                    name: commit["committer"]["name"].as_str().map(|s| s.to_string()),
                    email: commit["committer"]["email"].as_str().map(|s| s.to_string()),
                    avatar_url: commit["committer"]["avatar_url"].as_str().map(|s| s.to_string()),
                    html_url: commit["committer"]["html_url"].as_str().map(|s| s.to_string()),
                    type_: commit["committer"]["type"].as_str().map(|s| s.to_string()),
                    site_admin: commit["committer"]["site_admin"].as_bool().unwrap_or(false),
                },
                tree: GitTree {
                    sha: commit["tree"]["sha"].as_str().unwrap_or("").to_string(),
                    url: commit["tree"]["url"].as_str().unwrap_or("").to_string(),
                },
                parents: commit["parents"].as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|p| p["sha"].as_str().unwrap_or("").to_string())
                    .collect(),
                stats: commit["stats"].as_object().map(|stats| GitStats {
                    additions: stats["additions"].as_u64().unwrap_or(0) as u32,
                    deletions: stats["deletions"].as_u64().unwrap_or(0) as u32,
                    changed_files: stats["changed_files"].as_u64().unwrap_or(0) as u32,
                }),
                url: commit["url"].as_str().unwrap_or("").to_string(),
                html_url: commit["html_url"].as_str().unwrap_or("").to_string(),
                timestamp: DateTime::parse_from_rfc3339(commit["commit"]["author"]["date"].as_str().unwrap_or(""))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                added_files: vec![],
                modified_files: vec![],
                deleted_files: vec![],
            })
            .collect();
        
        Ok(commit_list)
    }

    async fn create_commit(&self, repository_path: &str, message: &str, files: Vec<GitFileChange>) -> Result<GitCommit> {
        // Use git command to create commit
        for file_change in files {
            match file_change.operation {
                GitFileOperation::Create | GitFileOperation::Update => {
                    let file_path = Path::new(repository_path).join(&file_change.path);
                    if let Some(parent) = file_path.parent() {
                        fs::create_dir_all(parent).map_err(|e| GitError::api(format!("Failed to create directory: {}", e)))?;
                    }
                    fs::write(&file_path, file_change.content)
                        .map_err(|e| GitError::api(format!("Failed to write file: {}", e)))?;
                }
                GitFileOperation::Delete => {
                    let file_path = Path::new(repository_path).join(&file_change.path);
                    fs::remove_file(&file_path)
                        .map_err(|e| GitError::api(format!("Failed to delete file: {}", e)))?;
                }
            }
        }
        
        // Add files and commit
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("add")
            .arg(".");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git add failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git add failed"));
        }
        
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("commit")
            .arg("-m")
            .arg(message);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git commit failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git commit failed"));
        }
        
        // Get the commit SHA
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("rev-parse")
            .arg("HEAD");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git rev-parse failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git rev-parse failed"));
        }
        
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        
        // Get commit details
        self.get_commit_from_sha(repository_path, &sha).await
    }

    async fn list_branches(&self, owner: &str, name: &str) -> Result<Vec<GitBranch>> {
        let request = self.build_request(&format!("/repos/{}/{}/branches", owner, name))?;
        let response = request.call()?;
        let branches: serde_json::Value = response.into_json()?;
        
        let branch_list: Vec<GitBranch> = branches.as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|branch| GitBranch {
                name: branch["name"].as_str().unwrap_or("").to_string(),
                commit: GitCommit {
                    sha: branch["commit"]["sha"].as_str().unwrap_or("").to_string(),
                    message: branch["commit"]["commit"]["message"].as_str().unwrap_or("").to_string(),
                    author: GitUser {
                        id: branch["commit"]["author"]["id"].as_u64().unwrap_or(0),
                        login: branch["commit"]["author"]["login"].as_str().unwrap_or("").to_string(),
                        name: branch["commit"]["author"]["name"].as_str().map(|s| s.to_string()),
                        email: branch["commit"]["author"]["email"].as_str().map(|s| s.to_string()),
                        avatar_url: branch["commit"]["author"]["avatar_url"].as_str().map(|s| s.to_string()),
                        html_url: branch["commit"]["author"]["html_url"].as_str().map(|s| s.to_string()),
                        type_: branch["commit"]["author"]["type"].as_str().map(|s| s.to_string()),
                        site_admin: branch["commit"]["author"]["site_admin"].as_bool().unwrap_or(false),
                    },
                    committer: GitUser {
                        id: branch["commit"]["committer"]["id"].as_u64().unwrap_or(0),
                        login: branch["commit"]["committer"]["login"].as_str().unwrap_or("").to_string(),
                        name: branch["commit"]["committer"]["name"].as_str().map(|s| s.to_string()),
                        email: branch["commit"]["committer"]["email"].as_str().map(|s| s.to_string()),
                        avatar_url: branch["commit"]["committer"]["avatar_url"].as_str().map(|s| s.to_string()),
                        html_url: branch["commit"]["committer"]["html_url"].as_str().map(|s| s.to_string()),
                        type_: branch["commit"]["committer"]["type"].as_str().map(|s| s.to_string()),
                        site_admin: branch["commit"]["committer"]["site_admin"].as_bool().unwrap_or(false),
                    },
                    tree: GitTree {
                        sha: branch["commit"]["tree"]["sha"].as_str().unwrap_or("").to_string(),
                        url: branch["commit"]["tree"]["url"].as_str().unwrap_or("").to_string(),
                    },
                    parents: branch["commit"]["parents"].as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .map(|p| p["sha"].as_str().unwrap_or("").to_string())
                        .collect(),
                    stats: None,
                    url: branch["commit"]["url"].as_str().unwrap_or("").to_string(),
                    html_url: branch["commit"]["html_url"].as_str().unwrap_or("").to_string(),
                    timestamp: DateTime::parse_from_rfc3339(branch["commit"]["author"]["date"].as_str().unwrap_or(""))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    added_files: vec![],
                    modified_files: vec![],
                    deleted_files: vec![],
                },
                protected: branch["protected"].as_bool().unwrap_or(false),
                default: branch["name"].as_str().unwrap_or("") == "main" || branch["name"].as_str().unwrap_or("") == "master",
                ahead: None,
                behind: None,
                upstream: None,
            })
            .collect();
        
        Ok(branch_list)
    }

    async fn create_branch(&self, repository_path: &str, branch_name: &str, from_branch: Option<&str>) -> Result<GitBranch> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("checkout")
            .arg("-b")
            .arg(branch_name);
        
        if let Some(from) = from_branch {
            cmd.arg(from);
        }
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git checkout failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to create branch"));
        }
        
        // Get branch details
        self.get_branch_from_name(repository_path, branch_name).await
    }

    async fn delete_branch(&self, repository_path: &str, branch_name: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("branch")
            .arg("-D")
            .arg(branch_name);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git branch delete failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to delete branch"));
        }
        
        Ok(())
    }

    async fn merge_branch(&self, repository_path: &str, source_branch: &str, target_branch: &str, options: GitMergeOptions) -> Result<GitMergeResult> {
        // Checkout target branch
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("checkout")
            .arg(target_branch);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git checkout failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to checkout target branch"));
        }
        
        // Merge source branch
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("merge")
            .arg(source_branch);
        
        if options.no_ff {
            cmd.arg("--no-ff");
        }
        
        if options.squash {
            cmd.arg("--squash");
        }
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git merge failed: {}", e)))?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Ok(GitMergeResult {
                success: false,
                merged_commit: None,
                conflicts: vec![error.to_string()],
                error_message: Some(error.to_string()),
                merge_time_ms: 0,
            });
        }
        
        // Get merged commit
        let merged_commit = self.get_current_commit(repository_path)?;
        
        Ok(GitMergeResult {
            success: true,
            merged_commit: Some(merged_commit),
            conflicts: vec![],
            error_message: None,
            merge_time_ms: 0,
        })
    }

    async fn list_tags(&self, owner: &str, name: &str) -> Result<Vec<GitTag>> {
        let request = self.build_request(&format!("/repos/{}/{}/tags", owner, name))?;
        let response = request.call()?;
        let tags: serde_json::Value = response.into_json()?;
        
        let tag_list: Vec<GitTag> = tags.as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|tag| GitTag {
                name: tag["name"].as_str().unwrap_or("").to_string(),
                commit: GitCommit {
                    sha: tag["commit"]["sha"].as_str().unwrap_or("").to_string(),
                    message: tag["commit"]["commit"]["message"].as_str().unwrap_or("").to_string(),
                    author: GitUser {
                        id: tag["commit"]["author"]["id"].as_u64().unwrap_or(0),
                        login: tag["commit"]["author"]["login"].as_str().unwrap_or("").to_string(),
                        name: tag["commit"]["author"]["name"].as_str().map(|s| s.to_string()),
                        email: tag["commit"]["author"]["email"].as_str().map(|s| s.to_string()),
                        avatar_url: tag["commit"]["author"]["avatar_url"].as_str().map(|s| s.to_string()),
                        html_url: tag["commit"]["author"]["html_url"].as_str().map(|s| s.to_string()),
                        type_: tag["commit"]["author"]["type"].as_str().map(|s| s.to_string()),
                        site_admin: tag["commit"]["author"]["site_admin"].as_bool().unwrap_or(false),
                    },
                    committer: GitUser {
                        id: tag["commit"]["committer"]["id"].as_u64().unwrap_or(0),
                        login: tag["commit"]["committer"]["login"].as_str().unwrap_or("").to_string(),
                        name: tag["commit"]["committer"]["name"].as_str().map(|s| s.to_string()),
                        email: tag["commit"]["committer"]["email"].as_str().map(|s| s.to_string()),
                        avatar_url: tag["commit"]["committer"]["avatar_url"].as_str().map(|s| s.to_string()),
                        html_url: tag["commit"]["committer"]["html_url"].as_str().map(|s| s.to_string()),
                        type_: tag["commit"]["committer"]["type"].as_str().map(|s| s.to_string()),
                        site_admin: tag["commit"]["committer"]["site_admin"].as_bool().unwrap_or(false),
                    },
                    tree: GitTree {
                        sha: tag["commit"]["tree"]["sha"].as_str().unwrap_or("").to_string(),
                        url: tag["commit"]["tree"]["url"].as_str().unwrap_or("").to_string(),
                    },
                    parents: tag["commit"]["parents"].as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .map(|p| p["sha"].as_str().unwrap_or("").to_string())
                        .collect(),
                    stats: None,
                    url: tag["commit"]["url"].as_str().unwrap_or("").to_string(),
                    html_url: tag["commit"]["html_url"].as_str().unwrap_or("").to_string(),
                    timestamp: DateTime::parse_from_rfc3339(tag["commit"]["author"]["date"].as_str().unwrap_or(""))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    added_files: vec![],
                    modified_files: vec![],
                    deleted_files: vec![],
                },
                tagger: tag["tagger"].as_object().map(|tagger| GitUser {
                    id: tagger["id"].as_u64().unwrap_or(0),
                    login: tagger["login"].as_str().unwrap_or("").to_string(),
                    name: tagger["name"].as_str().map(|s| s.to_string()),
                    email: tagger["email"].as_str().map(|s| s.to_string()),
                    avatar_url: tagger["avatar_url"].as_str().map(|s| s.to_string()),
                    html_url: tagger["html_url"].as_str().map(|s| s.to_string()),
                    type_: tagger["type"].as_str().map(|s| s.to_string()),
                    site_admin: tagger["site_admin"].as_bool().unwrap_or(false),
                }),
                message: tag["message"].as_str().map(|s| s.to_string()),
                zipball_url: tag["zipball_url"].as_str().unwrap_or("").to_string(),
                tarball_url: tag["tarball_url"].as_str().unwrap_or("").to_string(),
                timestamp: DateTime::parse_from_rfc3339(tag["tagger"]["date"].as_str().unwrap_or(""))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
            .collect();
        
        Ok(tag_list)
    }

    async fn create_tag(&self, repository_path: &str, tag_name: &str, target: &str, message: &str) -> Result<GitTag> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("tag")
            .arg("-a")
            .arg("-m")
            .arg(message)
            .arg(tag_name)
            .arg(target);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git tag failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to create tag"));
        }
        
        // Get tag details
        self.get_tag_from_name(repository_path, tag_name).await
    }

    async fn delete_tag(&self, repository_path: &str, tag_name: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("tag")
            .arg("-d")
            .arg(tag_name);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git tag delete failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to delete tag"));
        }
        
        Ok(())
    }

    async fn list_pull_requests(&self, owner: &str, name: &str, state: Option<GitPullRequestState>) -> Result<Vec<GitPullRequest>> {
        let mut path = format!("/repos/{}/{}/pulls", owner, name);
        
        if let Some(state) = state {
            let state_str = match state {
                GitPullRequestState::Open => "open",
                GitPullRequestState::Closed => "closed",
                GitPullRequestState::Merged => "merged",
            };
            path = format!("{}?state={}", path, state_str);
        }
        
        let request = self.build_request(&path)?;
        let response = request.call()?;
        let prs: serde_json::Value = response.into_json()?;
        
        let pr_list: Vec<GitPullRequest> = prs.as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|pr| GitPullRequest {
                id: pr["id"].as_u64().unwrap_or(0),
                number: pr["number"].as_u64().unwrap_or(0) as u32,
                title: pr["title"].as_str().unwrap_or("").to_string(),
                body: pr["body"].as_str().map(|s| s.to_string()),
                state: match pr["state"].as_str().unwrap_or("") {
                    "open" => GitPullRequestState::Open,
                    "closed" => GitPullRequestState::Closed,
                    "merged" => GitPullRequestState::Merged,
                    _ => GitPullRequestState::Closed,
                },
                user: GitUser {
                    id: pr["user"]["id"].as_u64().unwrap_or(0),
                    login: pr["user"]["login"].as_str().unwrap_or("").to_string(),
                    name: pr["user"]["name"].as_str().map(|s| s.to_string()),
                    email: pr["user"]["email"].as_str().map(|s| s.to_string()),
                    avatar_url: pr["user"]["avatar_url"].as_str().map(|s| s.to_string()),
                    html_url: pr["user"]["html_url"].as_str().map(|s| s.to_string()),
                    type_: pr["user"]["type"].as_str().map(|s| s.to_string()),
                    site_admin: pr["user"]["site_admin"].as_bool().unwrap_or(false),
                },
                assignee: pr["assignee"].as_object().map(|assignee| GitUser {
                    id: assignee["id"].as_u64().unwrap_or(0),
                    login: assignee["login"].as_str().unwrap_or("").to_string(),
                    name: assignee["name"].as_str().map(|s| s.to_string()),
                    email: assignee["email"].as_str().map(|s| s.to_string()),
                    avatar_url: assignee["avatar_url"].as_str().map(|s| s.to_string()),
                    html_url: assignee["html_url"].as_str().map(|s| s.to_string()),
                    type_: assignee["type"].as_str().map(|s| s.to_string()),
                    site_admin: assignee["site_admin"].as_bool().unwrap_or(false),
                }),
                head: GitPullRequestBranch {
                    label: pr["head"]["label"].as_str().unwrap_or("").to_string(),
                    ref_: pr["head"]["ref"].as_str().unwrap_or("").to_string(),
                    sha: pr["head"]["sha"].as_str().unwrap_or("").to_string(),
                    repo: GitRepository {
                        name: pr["head"]["repo"]["name"].as_str().unwrap_or("").to_string(),
                        url: pr["head"]["repo"]["html_url"].as_str().unwrap_or("").to_string(),
                        default_branch: pr["head"]["repo"]["default_branch"].as_str().unwrap_or("main").to_string(),
                        description: pr["head"]["repo"]["description"].as_str().map(|s| s.to_string()),
                        is_private: pr["head"]["repo"]["private"].as_bool().unwrap_or(false),
                        is_fork: pr["head"]["repo"]["fork"].as_bool().unwrap_or(false),
                        language: pr["head"]["repo"]["language"].as_str().map(|s| s.to_string()),
                        stargazers_count: pr["head"]["repo"]["stargazers_count"].as_u64().unwrap_or(0) as u32,
                        watchers_count: pr["head"]["repo"]["watchers_count"].as_u64().unwrap_or(0) as u32,
                        forks_count: pr["head"]["repo"]["forks_count"].as_u64().unwrap_or(0) as u32,
                        open_issues_count: pr["head"]["repo"]["open_issues_count"].as_u64().unwrap_or(0) as u32,
                        size: pr["head"]["repo"]["size"].as_u64().unwrap_or(0),
                        created_at: DateTime::parse_from_rfc3339(pr["head"]["repo"]["created_at"].as_str().unwrap_or(""))
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        updated_at: DateTime::parse_from_rfc3339(pr["head"]["repo"]["updated_at"].as_str().unwrap_or(""))
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        pushed_at: DateTime::parse_from_rfc3339(pr["head"]["repo"]["pushed_at"].as_str().unwrap_or(""))
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        owner: GitUser {
                            id: pr["head"]["repo"]["owner"]["id"].as_u64().unwrap_or(0),
                            login: pr["head"]["repo"]["owner"]["login"].as_str().unwrap_or("").to_string(),
                            name: pr["head"]["repo"]["owner"]["name"].as_str().map(|s| s.to_string()),
                            email: pr["head"]["repo"]["owner"]["email"].as_str().map(|s| s.to_string()),
                            avatar_url: pr["head"]["repo"]["owner"]["avatar_url"].as_str().map(|s| s.to_string()),
                            html_url: pr["head"]["repo"]["owner"]["html_url"].as_str().map(|s| s.to_string()),
                            type_: pr["head"]["repo"]["owner"]["type"].as_str().map(|s| s.to_string()),
                            site_admin: pr["head"]["repo"]["owner"]["site_admin"].as_bool().unwrap_or(false),
                        },
                    },
                },
                base: GitPullRequestBranch {
                    label: pr["base"]["label"].as_str().unwrap_or("").to_string(),
                    ref_: pr["base"]["ref"].as_str().unwrap_or("").to_string(),
                    sha: pr["base"]["sha"].as_str().unwrap_or("").to_string(),
                    repo: GitRepository {
                        name: pr["base"]["repo"]["name"].as_str().unwrap_or("").to_string(),
                        url: pr["base"]["repo"]["html_url"].as_str().unwrap_or("").to_string(),
                        default_branch: pr["base"]["repo"]["default_branch"].as_str().unwrap_or("main").to_string(),
                        description: pr["base"]["repo"]["description"].as_str().map(|s| s.to_string()),
                        is_private: pr["base"]["repo"]["private"].as_bool().unwrap_or(false),
                        is_fork: pr["base"]["repo"]["fork"].as_bool().unwrap_or(false),
                        language: pr["base"]["repo"]["language"].as_str().map(|s| s.to_string()),
                        stargazers_count: pr["base"]["repo"]["stargazers_count"].as_u64().unwrap_or(0) as u32,
                        watchers_count: pr["base"]["repo"]["watchers_count"].as_u64().unwrap_or(0) as u32,
                        forks_count: pr["base"]["repo"]["forks_count"].as_u64().unwrap_or(0) as u32,
                        open_issues_count: pr["base"]["repo"]["open_issues_count"].as_u64().unwrap_or(0) as u32,
                        size: pr["base"]["repo"]["size"].as_u64().unwrap_or(0),
                        created_at: DateTime::parse_from_rfc3339(pr["base"]["repo"]["created_at"].as_str().unwrap_or(""))
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        updated_at: DateTime::parse_from_rfc3339(pr["base"]["repo"]["updated_at"].as_str().unwrap_or(""))
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        pushed_at: DateTime::parse_from_rfc3339(pr["base"]["repo"]["pushed_at"].as_str().unwrap_or(""))
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        owner: GitUser {
                            id: pr["base"]["repo"]["owner"]["id"].as_u64().unwrap_or(0),
                            login: pr["base"]["repo"]["owner"]["login"].as_str().unwrap_or("").to_string(),
                            name: pr["base"]["repo"]["owner"]["name"].as_str().map(|s| s.to_string()),
                            email: pr["base"]["repo"]["owner"]["email"].as_str().map(|s| s.to_string()),
                            avatar_url: pr["base"]["repo"]["owner"]["avatar_url"].as_str().map(|s| s.to_string()),
                            html_url: pr["base"]["repo"]["owner"]["html_url"].as_str().map(|s| s.to_string()),
                            type_: pr["base"]["repo"]["owner"]["type"].as_str().map(|s| s.to_string()),
                            site_admin: pr["base"]["repo"]["owner"]["site_admin"].as_bool().unwrap_or(false),
                        },
                    },
                },
                merged: pr["merged"].as_bool().unwrap_or(false),
                mergeable: pr["mergeable"].as_bool(),
                created_at: DateTime::parse_from_rfc3339(pr["created_at"].as_str().unwrap_or(""))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(pr["updated_at"].as_str().unwrap_or(""))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                html_url: pr["html_url"].as_str().unwrap_or("").to_string(),
                diff_url: pr["diff_url"].as_str().unwrap_or("").to_string(),
                patch_url: pr["patch_url"].as_str().unwrap_or("").to_string(),
                review_comments: pr["review_comments"].as_u64().unwrap_or(0) as u32,
                commits: pr["commits"].as_u64().unwrap_or(0) as u32,
                additions: pr["additions"].as_u64().unwrap_or(0) as u32,
                deletions: pr["deletions"].as_u64().unwrap_or(0) as u32,
                changed_files: pr["changed_files"].as_u64().unwrap_or(0) as u32,
            })
            .collect();
        
        Ok(pr_list)
    }

    async fn create_pull_request(&self, owner: &str, name: &str, title: &str, body: &str, head: &str, base: &str) -> Result<GitPullRequest> {
        let pr_data = json!({
            "title": title,
            "body": body,
            "head": head,
            "base": base
        });
        
        let request = self.build_post_request(&format!("/repos/{}/{}/pulls", owner, name))?;
        let response = request.set("Content-Type", "application/json").send_json(&pr_data)?;
        
        let pr: serde_json::Value = response.into_json()?;
        
        Ok(GitPullRequest {
            id: pr["id"].as_u64().unwrap_or(0),
            number: pr["number"].as_u64().unwrap_or(0) as u32,
            title: pr["title"].as_str().unwrap_or("").to_string(),
            body: pr["body"].as_str().map(|s| s.to_string()),
            state: match pr["state"].as_str().unwrap_or("") {
                "open" => GitPullRequestState::Open,
                "closed" => GitPullRequestState::Closed,
                "merged" => GitPullRequestState::Merged,
                _ => GitPullRequestState::Open,
            },
            user: GitUser {
                id: pr["user"]["id"].as_u64().unwrap_or(0),
                login: pr["user"]["login"].as_str().unwrap_or("").to_string(),
                name: pr["user"]["name"].as_str().map(|s| s.to_string()),
                email: pr["user"]["email"].as_str().map(|s| s.to_string()),
                avatar_url: pr["user"]["avatar_url"].as_str().map(|s| s.to_string()),
                html_url: pr["user"]["html_url"].as_str().map(|s| s.to_string()),
                type_: pr["user"]["type"].as_str().map(|s| s.to_string()),
                site_admin: pr["user"]["site_admin"].as_bool().unwrap_or(false),
            },
            assignee: pr["assignee"].as_object().map(|assignee| GitUser {
                id: assignee["id"].as_u64().unwrap_or(0),
                login: assignee["login"].as_str().unwrap_or("").to_string(),
                name: assignee["name"].as_str().map(|s| s.to_string()),
                email: assignee["email"].as_str().map(|s| s.to_string()),
                avatar_url: assignee["avatar_url"].as_str().map(|s| s.to_string()),
                html_url: assignee["html_url"].as_str().map(|s| s.to_string()),
                type_: assignee["type"].as_str().map(|s| s.to_string()),
                site_admin: assignee["site_admin"].as_bool().unwrap_or(false),
            }),
            head: GitPullRequestBranch {
                label: pr["head"]["label"].as_str().unwrap_or("").to_string(),
                ref_: pr["head"]["ref"].as_str().unwrap_or("").to_string(),
                sha: pr["head"]["sha"].as_str().unwrap_or("").to_string(),
                repo: GitRepository {
                    name: pr["head"]["repo"]["name"].as_str().unwrap_or("").to_string(),
                    url: pr["head"]["repo"]["html_url"].as_str().unwrap_or("").to_string(),
                    default_branch: pr["head"]["repo"]["default_branch"].as_str().unwrap_or("main").to_string(),
                    description: pr["head"]["repo"]["description"].as_str().map(|s| s.to_string()),
                    is_private: pr["head"]["repo"]["private"].as_bool().unwrap_or(false),
                    is_fork: pr["head"]["repo"]["fork"].as_bool().unwrap_or(false),
                    language: pr["head"]["repo"]["language"].as_str().map(|s| s.to_string()),
                    stargazers_count: pr["head"]["repo"]["stargazers_count"].as_u64().unwrap_or(0) as u32,
                    watchers_count: pr["head"]["repo"]["watchers_count"].as_u64().unwrap_or(0) as u32,
                    forks_count: pr["head"]["repo"]["forks_count"].as_u64().unwrap_or(0) as u32,
                    open_issues_count: pr["head"]["repo"]["open_issues_count"].as_u64().unwrap_or(0) as u32,
                    size: pr["head"]["repo"]["size"].as_u64().unwrap_or(0),
                    created_at: DateTime::parse_from_rfc3339(pr["head"]["repo"]["created_at"].as_str().unwrap_or(""))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: DateTime::parse_from_rfc3339(pr["head"]["repo"]["updated_at"].as_str().unwrap_or(""))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    pushed_at: DateTime::parse_from_rfc3339(pr["head"]["repo"]["pushed_at"].as_str().unwrap_or(""))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    owner: GitUser {
                        id: pr["head"]["repo"]["owner"]["id"].as_u64().unwrap_or(0),
                        login: pr["head"]["repo"]["owner"]["login"].as_str().unwrap_or("").to_string(),
                        name: pr["head"]["repo"]["owner"]["name"].as_str().map(|s| s.to_string()),
                        email: pr["head"]["repo"]["owner"]["email"].as_str().map(|s| s.to_string()),
                        avatar_url: pr["head"]["repo"]["owner"]["avatar_url"].as_str().map(|s| s.to_string()),
                        html_url: pr["head"]["repo"]["owner"]["html_url"].as_str().map(|s| s.to_string()),
                        type_: pr["head"]["repo"]["owner"]["type"].as_str().map(|s| s.to_string()),
                        site_admin: pr["head"]["repo"]["owner"]["site_admin"].as_bool().unwrap_or(false),
                    },
                },
            },
            base: GitPullRequestBranch {
                label: pr["base"]["label"].as_str().unwrap_or("").to_string(),
                ref_: pr["base"]["ref"].as_str().unwrap_or("").to_string(),
                sha: pr["base"]["sha"].as_str().unwrap_or("").to_string(),
                repo: GitRepository {
                    name: pr["base"]["repo"]["name"].as_str().unwrap_or("").to_string(),
                    url: pr["base"]["repo"]["html_url"].as_str().unwrap_or("").to_string(),
                    default_branch: pr["base"]["repo"]["default_branch"].as_str().unwrap_or("main").to_string(),
                    description: pr["base"]["repo"]["description"].as_str().map(|s| s.to_string()),
                    is_private: pr["base"]["repo"]["private"].as_bool().unwrap_or(false),
                    is_fork: pr["base"]["repo"]["fork"].as_bool().unwrap_or(false),
                    language: pr["base"]["repo"]["language"].as_str().map(|s| s.to_string()),
                    stargazers_count: pr["base"]["repo"]["stargazers_count"].as_u64().unwrap_or(0) as u32,
                    watchers_count: pr["base"]["repo"]["watchers_count"].as_u64().unwrap_or(0) as u32,
                    forks_count: pr["base"]["repo"]["forks_count"].as_u64().unwrap_or(0) as u32,
                    open_issues_count: pr["base"]["repo"]["open_issues_count"].as_u64().unwrap_or(0) as u32,
                    size: pr["base"]["repo"]["size"].as_u64().unwrap_or(0),
                    created_at: DateTime::parse_from_rfc3339(pr["base"]["repo"]["created_at"].as_str().unwrap_or(""))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: DateTime::parse_from_rfc3339(pr["base"]["repo"]["updated_at"].as_str().unwrap_or(""))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    pushed_at: DateTime::parse_from_rfc3339(pr["base"]["repo"]["pushed_at"].as_str().unwrap_or(""))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    owner: GitUser {
                        id: pr["base"]["repo"]["owner"]["id"].as_u64().unwrap_or(0),
                        login: pr["base"]["repo"]["owner"]["login"].as_str().unwrap_or("").to_string(),
                        name: pr["base"]["repo"]["owner"]["name"].as_str().map(|s| s.to_string()),
                        email: pr["base"]["repo"]["owner"]["email"].as_str().map(|s| s.to_string()),
                        avatar_url: pr["base"]["repo"]["owner"]["avatar_url"].as_str().map(|s| s.to_string()),
                        html_url: pr["base"]["repo"]["owner"]["html_url"].as_str().map(|s| s.to_string()),
                        type_: pr["base"]["repo"]["owner"]["type"].as_str().map(|s| s.to_string()),
                        site_admin: pr["base"]["repo"]["owner"]["site_admin"].as_bool().unwrap_or(false),
                    },
                },
            },
            merged: pr["merged"].as_bool().unwrap_or(false),
            mergeable: pr["mergeable"].as_bool(),
            created_at: DateTime::parse_from_rfc3339(pr["created_at"].as_str().unwrap_or(""))
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(pr["updated_at"].as_str().unwrap_or(""))
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            html_url: pr["html_url"].as_str().unwrap_or("").to_string(),
            diff_url: pr["diff_url"].as_str().unwrap_or("").to_string(),
            patch_url: pr["patch_url"].as_str().unwrap_or("").to_string(),
            review_comments: pr["review_comments"].as_u64().unwrap_or(0) as u32,
            commits: pr["commits"].as_u64().unwrap_or(0) as u32,
            additions: pr["additions"].as_u64().unwrap_or(0) as u32,
            deletions: pr["deletions"].as_u64().unwrap_or(0) as u32,
            changed_files: pr["changed_files"].as_u64().unwrap_or(0) as u32,
        })
    }

    async fn list_workflows(&self, owner: &str, name: &str) -> Result<Vec<GitWorkflow>> {
        let request = self.build_request(&format!("/repos/{}/{}/actions/workflows", owner, name))?;
        let response = request.call()?;
        let workflows: serde_json::Value = response.into_json()?;
        
        let workflow_list: Vec<GitWorkflow> = workflows["workflows"].as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|workflow| GitWorkflow {
                id: workflow["id"].as_u64().unwrap_or(0),
                name: workflow["name"].as_str().unwrap_or("").to_string(),
                path: workflow["path"].as_str().unwrap_or("").to_string(),
                state: match workflow["state"].as_str().unwrap_or("") {
                    "active" => GitWorkflowState::Active,
                    "inactive" => GitWorkflowState::Inactive,
                    "deleted" => GitWorkflowState::Deleted,
                    _ => GitWorkflowState::Inactive,
                },
                created_at: DateTime::parse_from_rfc3339(workflow["created_at"].as_str().unwrap_or(""))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(workflow["updated_at"].as_str().unwrap_or(""))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                html_url: workflow["html_url"].as_str().unwrap_or("").to_string(),
                badge_url: workflow["badge_url"].as_str().unwrap_or("").to_string(),
            })
            .collect();
        
        Ok(workflow_list)
    }

    async fn trigger_workflow(&self, owner: &str, name: &str, workflow: &str, inputs: HashMap<String, String>) -> Result<GitWorkflowRun> {
        let trigger_data = json!({
            "ref": "main", // Would get from repository default branch
            "inputs": inputs
        });
        
        let request = self.build_post_request(&format!("/repos/{}/{}/actions/workflows/{}/dispatches", owner, name, workflow))?;
        let response = request.set("Content-Type", "application/json").send_json(&trigger_data)?;
        
        let run: serde_json::Value = response.into_json()?;
        
        Ok(GitWorkflowRun {
            id: run["id"].as_u64().unwrap_or(0),
            workflow: workflow.to_string(),
            name: run["name"].as_str().map(|s| s.to_string()),
            head_branch: run["head_branch"].as_str().unwrap_or("").to_string(),
            head_sha: run["head_sha"].as_str().unwrap_or("").to_string(),
            status: match run["status"].as_str().unwrap_or("") {
                "queued" => GitWorkflowStatus::Queued,
                "in_progress" => GitWorkflowStatus::InProgress,
                "completed" => GitWorkflowStatus::Completed,
                _ => GitWorkflowStatus::Queued,
            },
            conclusion: run["conclusion"].as_str().map(|c| match c {
                "success" => GitWorkflowConclusion::Success,
                "failure" => GitWorkflowConclusion::Failure,
                "neutral" => GitWorkflowConclusion::Neutral,
                "cancelled" => GitWorkflowConclusion::Cancelled,
                "skipped" => GitWorkflowConclusion::Skipped,
                "timed_out" => GitWorkflowConclusion::TimedOut,
                "action_required" => GitWorkflowConclusion::ActionRequired,
                _ => GitWorkflowConclusion::Failure,
            }),
            created_at: DateTime::parse_from_rfc3339(run["created_at"].as_str().unwrap_or(""))
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(run["updated_at"].as_str().unwrap_or(""))
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            started_at: run["run_started_at"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))),
            completed_at: run["completed_at"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))),
            url: run["url"].as_str().unwrap_or("").to_string(),
            html_url: run["html_url"].as_str().unwrap_or("").to_string(),
            jobs_url: run["jobs_url"].as_str().unwrap_or("").to_string(),
            logs_url: run["logs_url"].as_str().unwrap_or("").to_string(),
            check_suite_url: run["check_suite_url"].as_str().unwrap_or("").to_string(),
            run_number: run["run_number"].as_u64().unwrap_or(0),
            event: run["event"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn list_workflow_runs(&self, owner: &str, name: &str, workflow: Option<&str>) -> Result<Vec<GitWorkflowRun>> {
        let mut path = format!("/repos/{}/{}/actions/runs", owner, name);
        
        if let Some(workflow) = workflow {
            path = format!("{}?workflow={}", path, workflow);
        }
        
        let request = self.build_request(&path)?;
        let response = request.call()?;
        let runs: serde_json::Value = response.into_json()?;
        
        let run_list: Vec<GitWorkflowRun> = runs["workflow_runs"].as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|run| GitWorkflowRun {
                id: run["id"].as_u64().unwrap_or(0),
                workflow: run["name"].as_str().unwrap_or("").to_string(),
                name: run["name"].as_str().map(|s| s.to_string()),
                head_branch: run["head_branch"].as_str().unwrap_or("").to_string(),
                head_sha: run["head_sha"].as_str().unwrap_or("").to_string(),
                status: match run["status"].as_str().unwrap_or("") {
                    "queued" => GitWorkflowStatus::Queued,
                    "in_progress" => GitWorkflowStatus::InProgress,
                    "completed" => GitWorkflowStatus::Completed,
                    _ => GitWorkflowStatus::Queued,
                },
                conclusion: run["conclusion"].as_str().map(|c| match c {
                    "success" => GitWorkflowConclusion::Success,
                    "failure" => GitWorkflowConclusion::Failure,
                    "neutral" => GitWorkflowConclusion::Neutral,
                    "cancelled" => GitWorkflowConclusion::Cancelled,
                    "skipped" => GitWorkflowConclusion::Skipped,
                    "timed_out" => GitWorkflowConclusion::TimedOut,
                    "action_required" => GitWorkflowConclusion::ActionRequired,
                    _ => GitWorkflowConclusion::Failure,
                }),
                created_at: DateTime::parse_from_rfc3339(run["created_at"].as_str().unwrap_or(""))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(run["updated_at"].as_str().unwrap_or(""))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                started_at: run["run_started_at"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))),
                completed_at: run["completed_at"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))),
                url: run["url"].as_str().unwrap_or("").to_string(),
                html_url: run["html_url"].as_str().unwrap_or("").to_string(),
                jobs_url: run["jobs_url"].as_str().unwrap_or("").to_string(),
                logs_url: run["logs_url"].as_str().unwrap_or("").to_string(),
                check_suite_url: run["check_suite_url"].as_str().unwrap_or("").to_string(),
                run_number: run["run_number"].as_u64().unwrap_or(0),
                event: run["event"].as_str().unwrap_or("").to_string(),
            })
            .collect();
        
        Ok(run_list)
    }

    async fn get_file(&self, owner: &str, name: &str, path: &str, branch: Option<&str>) -> Result<Option<GitFile>> {
        let mut path = format!("/repos/{}/{}/contents/{}", owner, name, path);
        
        if let Some(branch) = branch {
            path = format!("{}?ref={}", path, branch);
        }
        
        let request = self.build_request(&path)?;
        let response = request.call()?;
        
        if response.status() == 404 {
            return Ok(None);
        }
        
        let file: serde_json::Value = response.into_json()?;
        
        Ok(Some(GitFile {
            path: file["path"].as_str().unwrap_or("").to_string(),
            name: file["name"].as_str().unwrap_or("").to_string(),
            sha: file["sha"].as_str().unwrap_or("").to_string(),
            size: file["size"].as_u64().unwrap_or(0),
            type_: file["type"].as_str().unwrap_or("").to_string(),
            encoding: file["encoding"].as_str().map(|s| s.to_string()),
            content: file["content"].as_str().map(|s| s.to_string()),
            download_url: file["download_url"].as_str().unwrap_or("").to_string(),
            html_url: file["html_url"].as_str().unwrap_or("").to_string(),
            git_url: file["git_url"].as_str().unwrap_or("").to_string(),
        }))
    }

    async fn create_file(&self, repository_path: &str, path: &str, content: &str, message: &str) -> Result<GitFile> {
        // Use git command to create file
        let file_path = Path::new(repository_path).join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| GitError::api(format!("Failed to create directory: {}", e)))?;
        }
        
        fs::write(&file_path, content)
            .map_err(|e| GitError::api(format!("Failed to write file: {}", e)))?;
        
        // Add and commit
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("add")
            .arg(path);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git add failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git add failed"));
        }
        
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("commit")
            .arg("-m")
            .arg(message);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git commit failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git commit failed"));
        }
        
        // Get file info
        self.get_file_from_path(repository_path, path).await
    }

    async fn delete_file(&self, repository_path: &str, path: &str, message: &str) -> Result<()> {
        // Use git command to delete file
        let file_path = Path::new(repository_path).join(path);
        
        fs::remove_file(&file_path)
            .map_err(|e| GitError::api(format!("Failed to delete file: {}", e)))?;
        
        // Add and commit
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("add")
            .arg(path);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git add failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git add failed"));
        }
        
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("commit")
            .arg("-m")
            .arg(message);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git commit failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git commit failed"));
        }
        
        Ok(())
    }

    async fn get_diff(&self, repository_path: &str, from: &str, to: &str) -> Result<Vec<GitDiff>> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("diff")
            .arg(from)
            .arg(to);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git diff failed: {}", e)))?;
        
        let diff_output = String::from_utf8_lossy(&output.stdout);
        let mut diffs = Vec::new();
        
        for line in diff_output.lines() {
            if line.starts_with("diff --git") {
                // Parse diff header
                continue;
            }
            // Parse diff content (simplified)
            diffs.push(GitDiff {
                file_path: line.to_string(),
                a_mode: "".to_string(),
                b_mode: "".to_string(),
                a_sha: None,
                b_sha: None,
                a_path: None,
                b_path: None,
                diff: line.to_string(),
                new_file: false,
                deleted_file: false,
                renamed_file: false,
            });
        }
        
        Ok(diffs)
    }

    async fn get_status(&self, repository_path: &str) -> Result<GitStatus> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("status")
            .arg("--porcelain");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git status failed: {}", e)))?;
        
        let status_output = String::from_utf8_lossy(&output.stdout);
        let mut status = GitStatus {
            modified: Vec::new(),
            added: Vec::new(),
            deleted: Vec::new(),
            renamed: Vec::new(),
            untracked: Vec::new(),
            branch: "".to_string(),
            ahead: None,
            behind: None,
            clean: true,
        };
        
        for line in status_output.lines() {
            if line.starts_with("##") {
                // Branch info
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 2 {
                    status.branch = parts[1].to_string();
                    
                    // Parse ahead/behind
                    for part in parts.iter() {
                        if part.starts_with("[ahead") {
                            if let Some(ahead) = part.strip_prefix("[ahead ").and_then(|s| s.strip_suffix("]")) {
                                status.ahead = ahead.parse().ok();
                            }
                        } else if part.starts_with("[behind") {
                            if let Some(behind) = part.strip_prefix("[behind ").and_then(|s| s.strip_suffix("]")) {
                                status.behind = behind.parse().ok();
                            }
                        }
                    }
                }
            } else if line.starts_with(" M ") {
                status.modified.push(line[2..].to_string());
                status.clean = false;
            } else if line.starts_with("A ") {
                status.added.push(line[2..].to_string());
                status.clean = false;
            } else if line.starts_with("D ") {
                status.deleted.push(line[2..].to_string());
                status.clean = false;
            } else if line.starts_with("R ") {
                // Renamed file
                let parts: Vec<&str> = line[2..].split_whitespace().collect();
                if parts.len() >= 2 {
                    status.renamed.push((parts[0].to_string(), parts[1].to_string()));
                }
                status.clean = false;
            } else if line.starts_with("?? ") {
                status.untracked.push(line[2..].to_string());
                status.clean = false;
            }
        }
        
        Ok(status)
    }

    // Helper methods for GitHub client
    fn get_current_commit(&self, repository_path: &str) -> Result<String> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("rev-parse")
            .arg("HEAD");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git rev-parse failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to get current commit"));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn get_current_branch(&self, repository_path: &str) -> Result<String> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git rev-parse failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to get current branch"));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn get_commit_from_sha(&self, repository_path: &str, sha: &str) -> Result<GitCommit> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("show")
            .arg("--format=fuller")
            .arg("--no-patch")
            .arg(sha);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git show failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to get commit details"));
        }
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        
        // Parse git show output (simplified)
        let lines: Vec<&str> = output_str.lines().collect();
        let mut commit = GitCommit {
            sha: sha.to_string(),
            message: "".to_string(),
            author: GitUser {
                id: 0,
                login: "".to_string(),
                name: None,
                email: None,
                avatar_url: None,
                html_url: None,
                type_: None,
                site_admin: false,
            },
            committer: GitUser {
                id: 0,
                login: "".to_string(),
                name: None,
                email: None,
                avatar_url: None,
                html_url: None,
                type_: None,
                site_admin: false,
            },
            tree: GitTree {
                sha: "".to_string(),
                url: "".to_string(),
            },
            parents: Vec::new(),
            stats: None,
            url: "".to_string(),
            html_url: "".to_string(),
            timestamp: Utc::now(),
            added_files: Vec::new(),
            modified_files: Vec::new(),
            deleted_files: Vec::new(),
        };
        
        for line in lines {
            if line.starts_with("commit ") {
                commit.sha = line[7..].to_string();
            } else if line.starts_with("Author:") {
                // Parse author info (simplified)
                let author_part = line[7..].trim();
                if let Some(email_start) = author_part.find('<') {
                    if let Some(email_end) = author_part.find('>') {
                        let name = author_part[..email_start].trim();
                        let email = &author_part[email_start + 1..email_end];
                        commit.author.name = Some(name.trim().to_string());
                        commit.author.email = Some(email.trim().to_string());
                    }
                }
            } else if line.starts_with("Date:") {
                let date_str = line[5..].trim();
                if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
                    commit.timestamp = dt.with_timezone(&Utc);
                }
            } else if line.starts_with("    ") {
                commit.message = line[4..].trim().to_string();
            }
        }
        
        Ok(commit)
    }

    async fn get_branch_from_name(&self, repository_path: &str, branch_name: &str) -> Result<GitBranch> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("show")
            .arg("--format=fuller")
            .arg("--no-patch")
            .arg(branch_name);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git show failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::branch_not_found(branch_name));
        }
        
        let commit = self.get_commit_from_sha(repository_path, branch_name).await?;
        
        Ok(GitBranch {
            name: branch_name.to_string(),
            commit,
            protected: false,
            default: false,
            ahead: None,
            behind: None,
            upstream: None,
        })
    }

    async fn get_tag_from_name(&self, repository_path: &str, tag_name: &str) -> Result<GitTag> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("show")
            .arg("--format=fuller")
            .arg("--no-patch")
            .arg(tag_name);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git show failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::tag_not_found(tag_name));
        }
        
        let commit = self.get_commit_from_sha(repository_path, tag_name).await?;
        
        Ok(GitTag {
            name: tag_name.to_string(),
            commit,
            tagger: None,
            message: None,
            zipball_url: "".to_string(),
            tarball_url: "".to_string(),
            timestamp: Utc::now(),
        })
    }

    async fn get_file_from_path(&self, repository_path: &str, path: &str) -> Result<GitFile> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("ls-tree")
            .arg("HEAD")
            .arg(path);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git ls-tree failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::file_not_found(path));
        }
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = output_str.split_whitespace().collect();
        
        if parts.len() >= 3 {
            let file_type = parts[1];
            let file_sha = parts[2];
            let file_name = parts[3..].join(" ");
            
            Ok(GitFile {
                path: path.to_string(),
                name: file_name,
                sha: file_sha.to_string(),
                size: 0,
                type_: file_type.to_string(),
                encoding: None,
                content: None,
                download_url: "".to_string(),
                html_url: "".to_string(),
                git_url: "".to_string(),
            })
        } else {
            Err(GitError::file_not_found(path))
        }
    }
}

impl Default for GitHubClient {
    fn default() -> Self {
        Self::new()
    }
}

// Local Git client (for local repository operations)
pub struct LocalGitClient {
    config: Option<GitConfig>,
}

impl LocalGitClient {
    pub fn new() -> Self {
        Self {
            config: None,
        }
    }
}

#[async_trait]
impl GitClient for LocalGitClient {
    async fn initialize(&mut self, config: GitConfig) -> Result<()> {
        self.config = Some(config);
        Ok(())
    }

    async fn test_connection(&self) -> Result<bool> {
        // Test if git is available
        let output = Command::new("git").arg("--version").output();
        Ok(output.is_ok())
    }

    async fn clone_repository(&self, repository_url: &str, destination: &str) -> Result<GitCloneResult> {
        let start_time = std::time::Instant::now();
        
        let mut cmd = Command::new("git");
        cmd.arg("clone").arg(repository_url).arg(destination);
        
        let output = cmd.output().map_err(|e| GitError::clone_failed(e.to_string()))?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Ok(GitCloneResult {
                repository_url: repository_url.to_string(),
                clone_path: destination.to_string(),
                cloned_to: destination.to_string(),
                commit: None,
                branch: None,
                success: false,
                error_message: Some(error.to_string()),
                clone_time_ms: start_time.elapsed().as_millis() as u64,
            });
        }
        
        let commit = self.get_current_commit(destination)?;
        let branch = self.get_current_branch(destination)?;
        
        Ok(GitCloneResult {
            repository_url: repository_url.to_string(),
            clone_path: destination.to_string(),
            cloned_to: destination.to_string(),
            commit,
            branch,
            success: true,
            error_message: None,
            clone_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    async fn get_repository(&self, _owner: &str, _name: &str) -> Result<Option<GitRepository>> {
        // Local Git client doesn't have repository metadata
        Ok(None)
    }

    async fn list_repositories(&self, _owner: Option<&str>, _organization: Option<&str>) -> Result<Vec<GitRepository>> {
        // Local Git client doesn't list repositories
        Ok(vec![])
    }

    async fn get_commit(&self, repository_path: &str, sha: &str) -> Result<Option<GitCommit>> {
        match self.get_commit_from_sha(repository_path, sha).await {
            Ok(commit) => Ok(Some(commit)),
            Err(_) => Ok(None),
        }
    }

    async fn list_commits(&self, repository_path: &str, branch: Option<&str>, limit: Option<u32>) -> Result<Vec<GitCommit>> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("log")
            .arg("--oneline")
            .arg("--format=%H");
        
        if let Some(branch) = branch {
            cmd.arg(branch);
        }
        
        if let Some(limit) = limit {
            cmd.arg(format!("-n{}", limit));
        }
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git log failed: {}", e)))?;
        
        let log_output = String::from_utf8_lossy(&output.stdout);
        let mut commits = Vec::new();
        
        for line in log_output.lines() {
            if !line.is_empty() {
                let commit = self.get_commit_from_sha(repository_path, line).await?;
                commits.push(commit);
            }
        }
        
        Ok(commits)
    }

    async fn create_commit(&self, repository_path: &str, message: &str, files: Vec<GitFileChange>) -> Result<GitCommit> {
        // Use git command to create commit
        for file_change in files {
            match file_change.operation {
                GitFileOperation::Create | GitFileOperation::Update => {
                    let file_path = Path::new(repository_path).join(&file_change.path);
                    if let Some(parent) = file_path.parent() {
                        fs::create_dir_all(parent).map_err(|e| GitError::api(format!("Failed to create directory: {}", e)))?;
                    }
                    fs::write(&file_path, file_change.content)
                        .map_err(|e| GitError::api(format!("Failed to write file: {}", e)))?;
                }
                GitFileOperation::Delete => {
                    let file_path = Path::new(repository_path).join(&file_change.path);
                    fs::remove_file(&file_path)
                        .map_err(|e| GitError::api(format!("Failed to delete file: {}", e)))?;
                }
            }
        }
        
        // Add files and commit
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("add")
            .arg(".");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git add failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git add failed"));
        }
        
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("commit")
            .arg("-m")
            .arg(message);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git commit failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git commit failed"));
        }
        
        // Get the commit SHA
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("rev-parse")
            .arg("HEAD");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git rev-parse failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git rev-parse failed"));
        }
        
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        
        // Get commit details
        self.get_commit_from_sha(repository_path, &sha).await
    }

    async fn list_branches(&self, repository_path: &str) -> Result<Vec<GitBranch>> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("branch")
            .arg("-a");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git branch failed: {}", e)))?;
        
        let branch_output = String::from_utf8_lossy(&output.stdout);
        let mut branches = Vec::new();
        
        for line in branch_output.lines() {
            let branch_name = line.trim().to_string();
            if !branch_name.starts_with("* ") {
                let commit = self.get_commit_from_sha(repository_path, &branch_name).await?;
                branches.push(GitBranch {
                    name: branch_name,
                    commit,
                    protected: false,
                    default: branch_name == "main" || branch_name == "master",
                    ahead: None,
                    behind: None,
                    upstream: None,
                });
            }
        }
        
        Ok(branches)
    }

    async fn create_branch(&self, repository_path: &str, branch_name: &str, from_branch: Option<&str>) -> Result<GitBranch> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("checkout")
            .arg("-b")
            .arg(branch_name);
        
        if let Some(from) = from_branch {
            cmd.arg(from);
        }
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git checkout failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to create branch"));
        }
        
        // Get branch details
        self.get_branch_from_name(repository_path, branch_name).await
    }

    async fn delete_branch(&self, repository_path: &str, branch_name: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("branch")
            .arg("-D")
            .arg(branch_name);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git branch delete failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to delete branch"));
        }
        
        Ok(())
    }

    async fn merge_branch(&self, repository_path: &str, source_branch: &str, target_branch: &str, options: GitMergeOptions) -> Result<GitMergeResult> {
        // Checkout target branch
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("checkout")
            .arg(target_branch);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git checkout failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to checkout target branch"));
        }
        
        // Merge source branch
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("merge")
            .arg(source_branch);
        
        if options.no_ff {
            cmd.arg("--no-ff");
        }
        
        if options.squash {
            cmd.arg("--squash");
        }
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git merge failed: {}", e)))?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Ok(GitMergeResult {
                success: false,
                merged_commit: None,
                conflicts: vec![error.to_string()],
                error_message: Some(error.to_string()),
                merge_time_ms: 0,
            });
        }
        
        // Get merged commit
        let merged_commit = self.get_current_commit(repository_path)?;
        
        Ok(GitMergeResult {
            success: true,
            merged_commit: Some(merged_commit),
            conflicts: vec![],
            error_message: None,
            merge_time_ms: 0,
        })
    }

    async fn list_tags(&self, repository_path: &str) -> Result<Vec<GitTag>> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("tag")
            .arg("-l");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git tag failed: {}", e)))?;
        
        let tag_output = String::from_utf8_lossy(&output.stdout);
        let mut tags = Vec::new();
        
        for line in tag_output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let tag_name = parts[0];
                let commit_sha = parts[1];
                
                let commit = self.get_commit_from_sha(repository_path, commit_sha).await?;
                tags.push(GitTag {
                    name: tag_name.to_string(),
                    commit,
                    tagger: None,
                    message: None,
                    zipball_url: "".to_string(),
                    tarball_url: "".to_string(),
                    timestamp: Utc::now(),
                });
            }
        }
        
        Ok(tags)
    }

    async fn create_tag(&self, repository_path: &str, tag_name: &str, target: &str, message: &str) -> Result<GitTag> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("tag")
            .arg("-a")
            .arg("-m")
            .arg(message)
            .arg(tag_name)
            .arg(target);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git tag failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to create tag"));
        }
        
        // Get tag details
        self.get_tag_from_name(repository_path, tag_name).await
    }

    async fn delete_tag(&self, repository_path: &str, tag_name: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("tag")
            .arg("-d")
            .arg(tag_name);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git tag delete failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to delete tag"));
        }
        
        Ok(())
    }

    async fn list_pull_requests(&self, _repository_path: &str, _owner: &str, _name: &str, _state: Option<GitPullRequestState>) -> Result<Vec<GitPullRequest>> {
        // Local Git client doesn't support pull requests
        Ok(vec![])
    }

    async fn create_pull_request(&self, _repository_path: &str, _owner: &str, _name: &str, _title: &str, _body: &str, _head: &str, _base: &str) -> Result<GitPullRequest> {
        // Local Git client doesn't support pull requests
        unimplemented!("Pull requests not supported by local Git client")
    }

    async fn list_workflows(&self, _repository_path: &str, _owner: &str, _name: &str) -> Result<Vec<GitWorkflow>> {
        // Local Git client doesn't support workflows
        Ok(vec![])
    }

    async fn trigger_workflow(&self, _repository_path: &str, _owner: &str, _name: &str, _workflow: &str, _inputs: HashMap<String, String>) -> Result<GitWorkflowRun> {
        // Local Git client doesn't support workflows
        unimplemented!("Workflows not supported by local Git client")
    }

    async fn list_workflow_runs(&self, _repository_path: &str, _owner: &str, _name: &str, _workflow: Option<&str>) -> Result<Vec<GitWorkflowRun>> {
        // Local Git client doesn't support workflow runs
        Ok(vec![])
    }

    async fn get_file(&self, repository_path: &str, _owner: &str, _name: &str, path: &str, _branch: Option<&str>) -> Result<Option<GitFile>> {
        match self.get_file_from_path(repository_path, path).await {
            Ok(file) => Ok(Some(file)),
            Err(_) => Ok(None),
        }
    }

    async fn create_file(&self, repository_path: &str, path: &str, content: &str, message: &str) -> Result<GitFile> {
        // Use git command to create file
        let file_path = Path::new(repository_path).join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| GitError::api(format!("Failed to create directory: {}", e)))?;
        }
        
        fs::write(&file_path, content)
            .map_err(|e| GitError::api(format!("Failed to write file: {}", e)))?;
        
        // Add and commit
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("add")
            .arg(path);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git add failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git add failed"));
        }
        
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("commit")
            .arg("-m")
            .arg(message);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git commit failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git commit failed"));
        }
        
        // Get file info
        self.get_file_from_path(repository_path, path).await
    }

    async fn delete_file(&self, repository_path: &str, path: &str, message: &str) -> Result<()> {
        // Use git command to delete file
        let file_path = Path::new(repository_path).join(path);
        
        fs::remove_file(&file_path)
            .map_err(|e| GitError::api(format!("Failed to delete file: {}", e)))?;
        
        // Add and commit
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("add")
            .arg(path);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git add failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git add failed"));
        }
        
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("commit")
            .arg("-m")
            .arg(message);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git commit failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Git commit failed"));
        }
        
        Ok(())
    }

    async fn get_diff(&self, repository_path: &str, from: &str, to: &str) -> Result<Vec<GitDiff>> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("diff")
            .arg(from)
            .arg(to);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git diff failed: {}", e)))?;
        
        let diff_output = String::from_utf8_lossy(&output.stdout);
        let mut diffs = Vec::new();
        
        for line in diff_output.lines() {
            if line.starts_with("diff --git") {
                // Parse diff header
                continue;
            }
            // Parse diff content (simplified)
            diffs.push(GitDiff {
                file_path: line.to_string(),
                a_mode: "".to_string(),
                b_mode: "".to_string(),
                a_sha: None,
                b_sha: None,
                a_path: None,
                b_path: None,
                diff: line.to_string(),
                new_file: false,
                deleted_file: false,
                renamed_file: false,
            });
        }
        
        Ok(diffs)
    }

    async fn get_status(&self, repository_path: &str) -> Result<GitStatus> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("status")
            .arg("--porcelain");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git status failed: {}", e)))?;
        
        let status_output = String::from_utf8_lossy(&output.stdout);
        let mut status = GitStatus {
            modified: Vec::new(),
            added: Vec::new(),
            deleted: Vec::new(),
            renamed: Vec::new(),
            untracked: Vec::new(),
            branch: "".to_string(),
            ahead: None,
            behind: None,
            clean: true,
        };
        
        for line in status_output.lines() {
            if line.starts_with("##") {
                // Branch info
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 2 {
                    status.branch = parts[1].to_string();
                    
                    // Parse ahead/behind
                    for part in parts.iter() {
                        if part.starts_with("[ahead") {
                            if let Some(ahead) = part.strip_prefix("[ahead ").and_then(|s| s.strip_suffix("]")) {
                                status.ahead = ahead.parse().ok();
                            }
                        } else if part.starts_with("[behind") {
                            if let Some(behind) = part.strip_prefix("[behind ").and_then(|s| s.strip_suffix("]")) {
                                status.behind = behind.parse().ok();
                            }
                        }
                    }
                }
            } else if line.starts_with("M ") {
                status.modified.push(line[2..].to_string());
                status.clean = false;
            } else if line.starts_with("A ") {
                status.added.push(line[2..].to_string());
                status.clean = false;
            } else if line.starts_with("D ") {
                status.deleted.push(line[2..].to_string());
                status.clean = false;
            } else if line.starts_with("R ") {
                // Renamed file
                let parts: Vec<&str> = line[2..].split_whitespace().collect();
                if parts.len() >= 2 {
                    status.renamed.push((parts[0].to_string(), parts[1].to_string()));
                }
                status.clean = false;
            } else if line.starts_with("?? ") {
                status.untracked.push(line[2..].to_string());
                status.clean = false;
            }
        }
        
        Ok(status)
    }

    // Helper methods for Local Git client
    fn get_current_commit(&self, repository_path: &str) -> Result<String> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("rev-parse")
            .arg("HEAD");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git rev-parse failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to get current commit"));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn get_current_branch(&self, repository_path: &str) -> Result<String> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD");
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git rev-parse failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to get current branch"));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn get_commit_from_sha(&self, repository_path: &str, sha: &str) -> Result<GitCommit> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("show")
            .arg("--format=fuller")
            .arg("--no-patch")
            .arg(sha);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git show failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::api("Failed to get commit details"));
        }
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        
        // Parse git show output (simplified)
        let lines: Vec<&str> = output_str.lines().collect();
        let mut commit = GitCommit {
            sha: sha.to_string(),
            message: "".to_string(),
            author: GitUser {
                id: 0,
                login: "".to_string(),
                name: None,
                email: None,
                avatar_url: None,
                html_url: None,
                type_: None,
                site_admin: false,
            },
            committer: GitUser {
                id: 0,
                login: "".to_string(),
                name: None,
                email: None,
                avatar_url: None,
                html_url: None,
                type_: None,
                site_admin: false,
            },
            tree: GitTree {
                sha: "".to_string(),
                url: "".to_string(),
            },
            parents: Vec::new(),
            stats: None,
            url: "".to_string(),
            html_url: "".to_string(),
            timestamp: Utc::now(),
            added_files: Vec::new(),
            modified_files: Vec::new(),
            deleted_files: Vec::new(),
        };
        
        for line in lines {
            if line.starts_with("commit ") {
                commit.sha = line[7..].to_string();
            } else if line.starts_with("Author:") {
                // Parse author info (simplified)
                let author_part = line[7..].trim();
                if let Some(email_start) = author_part.find('<') {
                    if let Some(email_end) = author_part.find('>') {
                        let name = author_part[..email_start].trim();
                        let email = &author_part[email_start + 1..email_end];
                        commit.author.name = Some(name.trim().to_string());
                        commit.author.email = Some(email.trim().to_string());
                    }
                }
            } else if line.starts_with("Date:") {
                let date_str = line[5..].trim();
                if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
                    commit.timestamp = dt.with_timezone(&Utc);
                }
            } else if line.starts_with("    ") {
                commit.message = line[4..].trim().to_string();
            }
        }
        
        Ok(commit)
    }

    async fn get_branch_from_name(&self, repository_path: &str, branch_name: &str) -> Result<GitBranch> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("show")
            .arg("--format=fuller")
            .arg("--no-patch")
            .arg(branch_name);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git show failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::branch_not_found(branch_name));
        }
        
        let commit = self.get_commit_from_sha(repository_path, branch_name).await?;
        
        Ok(GitBranch {
            name: branch_name.to_string(),
            commit,
            protected: false,
            default: false,
            ahead: None,
            behind: None,
            upstream: None,
        })
    }

    async fn get_tag_from_name(&self, repository_path: &str, tag_name: &str) -> Result<GitTag> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("show")
            .arg("--format=fuller")
            .arg("--no-patch")
            .arg(tag_name);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git show failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::tag_not_found(tag_name));
        }
        
        let commit = self.get_commit_from_sha(repository_path, tag_name).await?;
        
        Ok(GitTag {
            name: tag_name.to_string(),
            commit,
            tagger: None,
            message: None,
            zipball_url: "".to_string(),
            tarball_url: "".to_string(),
            timestamp: Utc::now(),
        })
    }

    async fn get_file_from_path(&self, repository_path: &str, path: &str) -> Result<GitFile> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repository_path)
            .arg("ls-tree")
            .arg("HEAD")
            .arg(path);
        
        let output = cmd.output().map_err(|e| GitError::api(format!("Git ls-tree failed: {}", e)))?;
        if !output.status.success() {
            return Err(GitError::file_not_found(path));
        }
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = output_str.split_whitespace().collect();
        
        if parts.len() >= 3 {
            let file_type = parts[1];
            let file_sha = parts[2];
            let file_name = parts[3..].join(" ");
            
            Ok(GitFile {
                path: path.to_string(),
                name: file_name,
                sha: file_sha.to_string(),
                size: 0,
                type_: file_type.to_string(),
                encoding: None,
                content: None,
                download_url: "".to_string(),
                html_url: "".to_string(),
                git_url: "".to_string(),
            })
        } else {
            Err(GitError::file_not_found(path))
        }
    }
}

impl Default for LocalGitClient {
    fn default() -> Self {
        Self::new()
    }
}