// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// Bot command framework with middleware support
// Provides command parsing, routing, and middleware pipeline
use super::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// Command framework
pub struct BotFramework {
    commands: Arc<RwLock<HashMap<String, Arc<dyn Command>>>>,
    middlewares: Arc<RwLock<Vec<Arc<dyn Middleware>>>>,
    prefix: Arc<RwLock<String>>,
    case_sensitive: Arc<RwLock<bool>>,
}

impl BotFramework {
    pub fn new() -> Self {
        Self {
            commands: Arc::new(RwLock::new(HashMap::new())),
            middlewares: Arc::new(RwLock::new(Vec::new())),
            prefix: Arc::new(RwLock::new("/".to_string())),
            case_sensitive: Arc::new(RwLock::new(false)),
        }
    }

    pub fn with_prefix(prefix: String) -> Self {
        let framework = Self::new();
        framework.set_prefix(prefix);
        framework
    }

    pub fn set_prefix(&self, prefix: String) {
        let mut p = self.prefix.write().unwrap();
        *p = prefix;
    }

    pub fn set_case_sensitive(&self, case_sensitive: bool) {
        let mut cs = self.case_sensitive.write().unwrap();
        *cs = case_sensitive;
    }

    pub fn register_command(&self, name: String, command: Arc<dyn Command>) {
        let mut commands = self.commands.write().unwrap();
        commands.insert(name, command);
    }

    pub fn add_middleware(&self, middleware: Arc<dyn Middleware>) {
        let mut middlewares = self.middlewares.write().unwrap();
        middlewares.push(middleware);
    }

    pub async fn process_message(&self, message: Message) -> Result<()> {
        // Parse command from message
        let command_context = self.parse_command(&message)?;
        if command_context.is_none() {
            return Ok(());
        }

        let mut ctx = command_context.unwrap();

        // Execute middleware pipeline
        for middleware in self.middlewares.read().unwrap().iter() {
            if let MiddlewareResult::Stop(reason) = middleware.before(&mut ctx).await? {
                return Ok(());
            }
        }

        // Execute command
        let result = self.execute_command(&ctx).await;

        // Execute after middleware
        for middleware in self.middlewares.read().unwrap().iter() {
            middleware.after(&mut ctx, &result).await?;
        }

        result
    }

    fn parse_command(&self, message: &Message) -> Result<Option<CommandContext>> {
        let content = match &message.content {
            MessageContent::Text(text) => text,
            _ => return Ok(None),
        };

        let prefix = self.prefix.read().unwrap();
        let case_sensitive = *self.case_sensitive.read().unwrap();

        // Check if message starts with command prefix
        let command_text = if content.starts_with(&**prefix) {
            &content[prefix.len()..]
        } else {
            return Ok(None);
        };

        // Split command and arguments
        let parts: Vec<&str> = command_text.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(None);
        }

        let command_name = if case_sensitive {
            parts[0].to_string()
        } else {
            parts[0].to_lowercase()
        };

        let arguments = parts[1..].iter().map(|s| s.to_string()).collect();

        Ok(Some(CommandContext {
            command_name,
            arguments,
            message: message.clone(),
            user_data: HashMap::new(),
            response_data: None,
            execution_time: None,
        }))
    }

    async fn execute_command(&self, ctx: &CommandContext) -> Result<()> {
        let commands = self.commands.read().unwrap();

        if let Some(command) = commands.get(&ctx.command_name) {
            command.execute(ctx.clone()).await?;
        }

        Ok(())
    }

    pub fn list_commands(&self) -> Vec<String> {
        let commands = self.commands.read().unwrap();
        commands.keys().cloned().collect()
    }

    pub fn get_command_info(&self, name: &str) -> Option<CommandInfo> {
        let commands = self.commands.read().unwrap();
        commands.get(name).map(|cmd| cmd.get_info())
    }
}

impl Default for BotFramework {
    fn default() -> Self {
        Self::new()
    }
}

// Command context
#[derive(Debug, Clone)]
pub struct CommandContext {
    pub command_name: String,
    pub arguments: Vec<String>,
    pub message: Message,
    pub user_data: HashMap<String, String>,
    pub response_data: Option<serde_json::Value>,
    pub execution_time: Option<std::time::Duration>,
}

impl CommandContext {
    pub fn get_argument(&self, index: usize) -> Option<&String> {
        self.arguments.get(index)
    }

    pub fn get_argument_or_default(&self, index: usize, default: &str) -> String {
        self.arguments
            .get(index)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    pub fn has_argument(&self, index: usize) -> bool {
        index < self.arguments.len()
    }

    pub fn argument_count(&self) -> usize {
        self.arguments.len()
    }

    pub fn set_user_data(&mut self, key: String, value: String) {
        self.user_data.insert(key, value);
    }

    pub fn get_user_data(&self, key: &str) -> Option<&String> {
        self.user_data.get(key)
    }

    pub fn get_sender_id(&self) -> &str {
        &self.message.sender_id
    }

    pub fn get_chat_id(&self) -> &str {
        &self.message.chat_id
    }
}

// Command trait
#[async_trait]
pub trait Command: Send + Sync {
    async fn execute(&self, ctx: CommandContext) -> Result<()>;
    fn get_info(&self) -> CommandInfo;
}

// Command information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub aliases: Vec<String>,
    pub required_permissions: Vec<String>,
    pub category: String,
    pub admin_only: bool,
}

// Middleware trait
#[async_trait]
pub trait Middleware: Send + Sync {
    async fn before(&self, ctx: &mut CommandContext) -> Result<MiddlewareResult>;
    async fn after(&self, ctx: &CommandContext, result: &Result<()>) -> Result<()>;
}

// Middleware execution result
pub enum MiddlewareResult {
    Continue,
    Stop(String),
    Response(String),
}

// Built-in middleware implementations

// Permission middleware
pub struct PermissionMiddleware {
    admin_users: Arc<RwLock<Vec<String>>>,
}

impl PermissionMiddleware {
    pub fn new(admin_users: Vec<String>) -> Self {
        Self {
            admin_users: Arc::new(RwLock::new(admin_users)),
        }
    }

    pub fn add_admin(&self, user_id: String) {
        let mut admins = self.admin_users.write().unwrap();
        admins.push(user_id);
    }

    pub fn remove_admin(&self, user_id: &str) {
        let mut admins = self.admin_users.write().unwrap();
        admins.retain(|id| id != user_id);
    }

    fn is_admin(&self, user_id: &str) -> bool {
        let admins = self.admin_users.read().unwrap();
        admins.contains(&user_id.to_string())
    }
}

#[async_trait]
impl Middleware for PermissionMiddleware {
    async fn before(&self, ctx: &mut CommandContext) -> Result<MiddlewareResult> {
        let commands = ctx
            .message
            .metadata
            .get("command_info")
            .cloned()
            .unwrap_or_default();
        let command_info: CommandInfo =
            serde_json::from_str(&commands).unwrap_or_else(|_| CommandInfo {
                name: ctx.command_name.clone(),
                description: String::new(),
                usage: String::new(),
                aliases: vec![],
                required_permissions: vec![],
                category: String::new(),
                admin_only: false,
            });

        if command_info.admin_only && !self.is_admin(&ctx.message.sender_id) {
            return Ok(MiddlewareResult::Stop("Admin only command".to_string()));
        }

        // Check required permissions
        for permission in command_info.required_permissions {
            if !self
                .has_permission(&ctx.message.sender_id, &permission)
                .await
            {
                return Ok(MiddlewareResult::Stop(format!(
                    "Missing permission: {}",
                    permission
                )));
            }
        }

        Ok(MiddlewareResult::Continue)
    }

    async fn after(&self, _ctx: &CommandContext, _result: &Result<()>) -> Result<()> {
        Ok(())
    }
}

impl PermissionMiddleware {
    async fn has_permission(&self, _user_id: &str, _permission: &str) -> bool {
        // This would typically check against a database or external service
        // For now, return true (allow all)
        true
    }
}

// Rate limiting middleware
pub struct RateLimitMiddleware {
    user_limits: Arc<RwLock<HashMap<String, UserRateLimit>>>,
    global_limit: Arc<RwLock<GlobalRateLimit>>,
}

impl RateLimitMiddleware {
    pub fn new(max_commands_per_minute: u32) -> Self {
        Self {
            user_limits: Arc::new(RwLock::new(HashMap::new())),
            global_limit: Arc::new(RwLock::new(GlobalRateLimit::new(max_commands_per_minute))),
        }
    }

    fn check_user_limit(&self, user_id: &str) -> Result<()> {
        let mut limits = self.user_limits.write().unwrap();
        let user_limit = limits
            .entry(user_id.to_string())
            .or_insert_with(|| UserRateLimit::new(5)); // 5 commands per minute per user

        user_limit.check_rate_limit()
    }

    fn check_global_limit(&self) -> Result<()> {
        let mut global = self.global_limit.write().unwrap();
        global.check_rate_limit()
    }
}

#[async_trait]
impl Middleware for RateLimitMiddleware {
    async fn before(&self, ctx: &mut CommandContext) -> Result<MiddlewareResult> {
        // Check user-specific limit
        if let Err(e) = self.check_user_limit(&ctx.message.sender_id) {
            return Ok(MiddlewareResult::Stop(format!(
                "Rate limit exceeded: {}",
                e
            )));
        }

        // Check global limit
        if let Err(e) = self.check_global_limit() {
            return Ok(MiddlewareResult::Stop(format!(
                "Global rate limit exceeded: {}",
                e
            )));
        }

        Ok(MiddlewareResult::Continue)
    }

    async fn after(&self, _ctx: &CommandContext, _result: &Result<()>) -> Result<()> {
        Ok(())
    }
}

struct UserRateLimit {
    max_per_minute: u32,
    requests: std::collections::VecDeque<u64>,
}

impl UserRateLimit {
    fn new(max_per_minute: u32) -> Self {
        Self {
            max_per_minute,
            requests: std::collections::VecDeque::new(),
        }
    }

    fn check_rate_limit(&mut self) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let window_start = now.saturating_sub(60);
        while let Some(&request_time) = self.requests.front() {
            if request_time < window_start {
                self.requests.pop_front();
            } else {
                break;
            }
        }

        if self.requests.len() as u32 >= self.max_per_minute {
            return Err(anyhow::anyhow!("Rate limit exceeded"));
        }

        self.requests.push_back(now);
        Ok(())
    }
}

struct GlobalRateLimit {
    max_per_minute: u32,
    requests: std::collections::VecDeque<u64>,
}

impl GlobalRateLimit {
    fn new(max_per_minute: u32) -> Self {
        Self {
            max_per_minute,
            requests: std::collections::VecDeque::new(),
        }
    }

    fn check_rate_limit(&mut self) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let window_start = now.saturating_sub(60);
        while let Some(&request_time) = self.requests.front() {
            if request_time < window_start {
                self.requests.pop_front();
            } else {
                break;
            }
        }

        if self.requests.len() as u32 >= self.max_per_minute {
            return Err(anyhow::anyhow!("Global rate limit exceeded"));
        }

        self.requests.push_back(now);
        Ok(())
    }
}

// Logging middleware
pub struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn before(&self, ctx: &mut CommandContext) -> Result<MiddlewareResult> {
        println!(
            "Command '{}' from user {} in chat {}",
            ctx.command_name, ctx.message.sender_id, ctx.message.chat_id
        );

        ctx.set_user_data(
            "start_time".to_string(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .to_string(),
        );

        Ok(MiddlewareResult::Continue)
    }

    async fn after(&self, ctx: &CommandContext, result: &Result<()>) -> Result<()> {
        let status = if result.is_ok() { "SUCCESS" } else { "ERROR" };

        println!(
            "Command '{}' completed with status: {}",
            ctx.command_name, status
        );

        Ok(())
    }
}

// Example simple command implementations
pub struct HelpCommand {
    bot_framework: Arc<BotFramework>,
}

impl HelpCommand {
    pub fn new(bot_framework: Arc<BotFramework>) -> Self {
        Self { bot_framework }
    }
}

#[async_trait]
impl Command for HelpCommand {
    async fn execute(&self, ctx: CommandContext) -> Result<()> {
        let commands = self.bot_framework.list_commands();
        let mut help_text = "Available commands:\n".to_string();

        for command in commands {
            if let Some(info) = self.bot_framework.get_command_info(&command) {
                help_text.push_str(&format!("• {} - {}\n", info.name, info.description));
            }
        }

        // This would typically send the help message back to the user
        println!("Sending help message: {}", help_text);

        Ok(())
    }

    fn get_info(&self) -> CommandInfo {
        CommandInfo {
            name: "help".to_string(),
            description: "Show available commands".to_string(),
            usage: "/help [command]".to_string(),
            aliases: vec!["h".to_string(), "?".to_string()],
            required_permissions: vec![],
            category: "Utility".to_string(),
            admin_only: false,
        }
    }
}

pub struct PingCommand;

#[async_trait]
impl Command for PingCommand {
    async fn execute(&self, ctx: CommandContext) -> Result<()> {
        let response = "Pong!";
        println!("Sending pong response to user {}", ctx.message.sender_id);
        Ok(())
    }

    fn get_info(&self) -> CommandInfo {
        CommandInfo {
            name: "ping".to_string(),
            description: "Check if the bot is responding".to_string(),
            usage: "/ping".to_string(),
            aliases: vec![],
            required_permissions: vec![],
            category: "Utility".to_string(),
            admin_only: false,
        }
    }
}
