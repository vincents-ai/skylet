// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod adapters;
pub mod bot_framework;
pub mod session_management;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export types from submodules
pub use adapters::{DiscordAdapter, SlackAdapter, TelegramAdapter};
pub use bot_framework::{
    BotFramework, Command, CommandContext, CommandInfo, HelpCommand, LoggingMiddleware, Middleware,
    MiddlewareResult, PermissionMiddleware, PingCommand, RateLimitMiddleware,
};
pub use session_management::{
    ConversationContext, ConversationFlow, ConversationState, ConversationStep,
    InMemorySessionStorage, PreferenceValue, SessionManager, SessionStorage, UserPreference,
    UserSession,
};

/// Core messaging platform trait
#[async_trait]
pub trait MessagingPlatform: Send + Sync {
    fn platform_name(&self) -> &str;
    fn platform_version(&self) -> &str;
    async fn initialize(&mut self, config: &PlatformConfig) -> anyhow::Result<()>;
    async fn is_healthy(&self) -> anyhow::Result<bool>;
    async fn send_message(&self, recipient: &str, content: &str) -> anyhow::Result<()>;
    async fn send_message_with_options(
        &self,
        recipient: &str,
        content: &str,
        options: MessageOptions,
    ) -> anyhow::Result<()>;
    async fn send_buttons(
        &self,
        recipient: &str,
        content: &str,
        buttons: Vec<InlineButton>,
    ) -> anyhow::Result<()>;
    async fn send_media(&self, recipient: &str, media: MediaContent) -> anyhow::Result<()>;
    async fn edit_message(&self, message_id: &str, new_content: &str) -> anyhow::Result<()>;
    async fn edit_message_buttons(
        &self,
        message_id: &str,
        buttons: Vec<InlineButton>,
    ) -> anyhow::Result<()>;
    async fn delete_message(&self, message_id: &str) -> anyhow::Result<()>;
    async fn get_user_info(&self, user_id: &str) -> anyhow::Result<Option<User>>;
    async fn get_chat_info(&self, chat_id: &str) -> anyhow::Result<Option<Chat>>;
    async fn start_listening(&self) -> anyhow::Result<()>;
    async fn stop_listening(&self) -> anyhow::Result<()>;
    fn get_capabilities(&self) -> PlatformCapabilities;
}

/// Platform configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    pub bot_token: String,
    pub webhook_url: Option<String>,
    pub max_retries: u32,
    pub timeout_seconds: u64,
    pub extra: HashMap<String, String>,
}

impl Default for PlatformConfig {
    fn default() -> Self {
        Self {
            bot_token: String::new(),
            webhook_url: None,
            max_retries: 3,
            timeout_seconds: 30,
            extra: HashMap::new(),
        }
    }
}

/// Message options for sending
#[derive(Debug, Clone, Default)]
pub struct MessageOptions {
    pub parse_mode: Option<ParseMode>,
    pub disable_preview: bool,
    pub disable_notification: bool,
    pub reply_to: Option<String>,
}

/// Parse mode for message formatting
#[derive(Debug, Clone, Copy)]
pub enum ParseMode {
    Markdown,
    MarkdownV2,
    HTML,
}

/// Inline button for keyboards
#[derive(Debug, Clone)]
pub struct InlineButton {
    pub text: String,
    pub url: Option<String>,
    pub callback_data: Option<String>,
}

/// Media content for sending
#[derive(Debug, Clone)]
pub struct MediaContent {
    pub media_type: MediaType,
    pub file_id: Option<String>,
    pub url: Option<String>,
    pub caption: Option<String>,
}

/// Media type enumeration
#[derive(Debug, Clone, Copy)]
pub enum MediaType {
    Photo,
    Video,
    Audio,
    Document,
    Voice,
    VideoNote,
    Animation,
    Sticker,
}

/// User information
#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub display_name: String,
    pub is_bot: bool,
    pub is_verified: bool,
    pub is_premium: bool,
    pub language_code: Option<String>,
    pub platform_specific: HashMap<String, String>,
}

/// Chat information
#[derive(Debug, Clone)]
pub struct Chat {
    pub id: String,
    pub chat_type: ChatType,
    pub title: Option<String>,
    pub username: Option<String>,
    pub description: Option<String>,
    pub invite_link: Option<String>,
    pub permissions: Option<ChatPermissions>,
    pub member_count: Option<u32>,
    pub platform_specific: HashMap<String, String>,
}

/// Chat type enumeration
#[derive(Debug, Clone, Copy)]
pub enum ChatType {
    Private,
    Group,
    Supergroup,
    Channel,
}

/// Chat permissions
#[derive(Debug, Clone)]
pub struct ChatPermissions {
    pub can_send_messages: bool,
    pub can_send_media: bool,
    pub can_send_polls: bool,
    pub can_add_web_page_previews: bool,
    pub can_change_info: bool,
    pub can_invite_users: bool,
    pub can_pin_messages: bool,
}

impl Default for ChatPermissions {
    fn default() -> Self {
        Self {
            can_send_messages: true,
            can_send_media: true,
            can_send_polls: true,
            can_add_web_page_previews: true,
            can_change_info: true,
            can_invite_users: true,
            can_pin_messages: true,
        }
    }
}

/// Platform capabilities
#[derive(Debug, Clone)]
pub struct PlatformCapabilities {
    pub supports_markdown: bool,
    pub supports_html: bool,
    pub supports_inline_buttons: bool,
    pub supports_reply_keyboards: bool,
    pub supports_media: Vec<MediaType>,
    pub supports_location: bool,
    pub supports_contact: bool,
    pub supports_polls: bool,
    pub supports_webhooks: bool,
    pub supports_long_polling: bool,
    pub supports_message_editing: bool,
    pub supports_message_deletion: bool,
    pub max_message_length: Option<usize>,
    pub max_media_size_mb: Option<usize>,
    pub rate_limit_per_minute: u32,
}

/// Message structure
#[derive(Debug, Clone)]
pub struct Message {
    pub id: String,
    pub sender_id: String,
    pub chat_id: String,
    pub content: MessageContent,
    pub timestamp: i64,
    pub reply_to: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Message content enumeration
#[derive(Debug, Clone)]
pub enum MessageContent {
    Text(String),
    Photo {
        file_id: String,
        caption: Option<String>,
    },
    Video {
        file_id: String,
        caption: Option<String>,
    },
    Audio {
        file_id: String,
        caption: Option<String>,
    },
    Document {
        file_id: String,
        caption: Option<String>,
    },
    Voice {
        file_id: String,
    },
    VideoNote {
        file_id: String,
    },
    Sticker {
        file_id: String,
    },
    Animation {
        file_id: String,
        caption: Option<String>,
    },
    Location {
        latitude: f64,
        longitude: f64,
    },
    Contact {
        phone_number: String,
        name: String,
    },
    Poll {
        question: String,
        options: Vec<String>,
    },
}

impl Message {
    pub fn text(
        id: impl Into<String>,
        sender_id: impl Into<String>,
        chat_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            sender_id: sender_id.into(),
            chat_id: chat_id.into(),
            content: MessageContent::Text(text.into()),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            reply_to: None,
            metadata: HashMap::new(),
        }
    }
}

/// Result type for messaging operations
pub type Result<T> = anyhow::Result<T>;
