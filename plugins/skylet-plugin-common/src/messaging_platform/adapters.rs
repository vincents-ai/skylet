// Platform-specific adapters for messaging platforms
// Provides implementations for Telegram, Discord, Slack, etc.
use super::*;
use async_trait::async_trait;
use serde_json::json;

// Telegram adapter implementation
pub struct TelegramAdapter {
    client: Option<ureq::Agent>,
    bot_token: String,
    config: Option<PlatformConfig>,
}

impl TelegramAdapter {
    pub fn new(bot_token: String) -> Self {
        Self {
            client: None,
            bot_token,
            config: None,
        }
    }

    fn build_url(&self, method: &str) -> String {
        format!("https://api.telegram.org/bot{}/{}", self.bot_token, method)
    }

    fn make_request(&self, method: &str, payload: &serde_json::Value) -> Result<serde_json::Value> {
        let client = self.client.as_ref().unwrap();
        let url = self.build_url(method);
        
        let response = client.post(&url)
            .set("Content-Type", "application/json")
            .send_string(&serde_json::to_string(payload)?)?;
        
        let response_text = response.into_string()?;
        let response: serde_json::Value = serde_json::from_str(&response_text)?;
        
        if response["ok"].as_bool() == Some(true) {
            Ok(response)
        } else {
            Err(anyhow::anyhow!("Telegram API error: {}", response))
        }
    }
}

#[async_trait]
impl MessagingPlatform for TelegramAdapter {
    fn platform_name(&self) -> &str {
        "telegram"
    }

    fn platform_version(&self) -> &str {
        "7.0"
    }

    async fn initialize(&mut self, config: &PlatformConfig) -> Result<()> {
        self.client = Some(ureq::AgentBuilder::new().build());
        self.config = Some(config.clone());
        Ok(())
    }

    async fn is_healthy(&self) -> Result<bool> {
        if self.client.is_none() {
            return Ok(false);
        }

        match self.make_request("getMe", &json!({})) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn send_message(&self, recipient: &str, content: &str) -> Result<()> {
        let payload = json!({
            "chat_id": recipient,
            "text": content,
        });
        
        self.make_request("sendMessage", &payload)?;
        Ok(())
    }

    async fn send_message_with_options(
        &self,
        recipient: &str,
        content: &str,
        options: MessageOptions,
    ) -> Result<()> {
        let mut payload = json!({
            "chat_id": recipient,
            "text": content,
        });

        // Add parse mode
        if let Some(parse_mode) = options.parse_mode {
            payload["parse_mode"] = json!(match parse_mode {
                ParseMode::Markdown => "Markdown",
                ParseMode::MarkdownV2 => "MarkdownV2", 
                ParseMode::HTML => "HTML",
            });
        }

        // Add disable preview
        if options.disable_preview {
            payload["disable_web_page_preview"] = json!(true);
        }

        // Add disable notification
        if options.disable_notification {
            payload["disable_notification"] = json!(true);
        }

        // Add reply to
        if let Some(reply_to) = options.reply_to {
            payload["reply_to_message_id"] = json!(reply_to);
        }

        self.make_request("sendMessage", &payload)?;
        Ok(())
    }

    async fn send_buttons(
        &self,
        recipient: &str,
        content: &str,
        buttons: Vec<InlineButton>,
    ) -> Result<()> {
        let keyboard: Vec<serde_json::Value> = buttons
            .into_iter()
            .map(|btn| {
                let mut btn_json = json!({
                    "text": btn.text,
                });

                if let Some(url) = btn.url {
                    btn_json["url"] = json!(url);
                }

                if let Some(callback_data) = btn.callback_data {
                    btn_json["callback_data"] = json!(callback_data);
                }

                btn_json
            })
            .collect();

        let payload = json!({
            "chat_id": recipient,
            "text": content,
            "reply_markup": {
                "inline_keyboard": [keyboard]
            }
        });

        self.make_request("sendMessage", &payload)?;
        Ok(())
    }

    async fn send_media(&self, recipient: &str, media: MediaContent) -> Result<()> {
        let method = match media.media_type {
            MediaType::Photo => "sendPhoto",
            MediaType::Video => "sendVideo",
            MediaType::Audio => "sendAudio",
            MediaType::Document => "sendDocument",
            MediaType::Voice => "sendVoice",
            MediaType::VideoNote => "sendVideoNote",
            MediaType::Animation => "sendAnimation",
            MediaType::Sticker => "sendSticker",
        };

        let mut payload = json!({
            "chat_id": recipient,
        });

        // Add media (file_id or URL)
        if let Some(file_id) = media.file_id {
            payload[method[4..].to_ascii_lowercase()] = json!(file_id);
        } else if let Some(url) = media.url {
            payload[method[4..].to_ascii_lowercase()] = json!(url);
        }

        // Add caption if provided
        if let Some(caption) = media.caption {
            payload["caption"] = json!(caption);
        }

        self.make_request(method, &payload)?;
        Ok(())
    }

    async fn edit_message(&self, message_id: &str, new_content: &str) -> Result<()> {
        // For Telegram, message_id should be in format "chat_id:message_id"
        let parts: Vec<&str> = message_id.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid message_id format. Expected 'chat_id:message_id'"));
        }

        let payload = json!({
            "chat_id": parts[0],
            "message_id": parts[1],
            "text": new_content,
        });

        self.make_request("editMessageText", &payload)?;
        Ok(())
    }

    async fn edit_message_buttons(&self, message_id: &str, buttons: Vec<InlineButton>) -> Result<()> {
        let parts: Vec<&str> = message_id.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid message_id format. Expected 'chat_id:message_id'"));
        }

        let keyboard: Vec<serde_json::Value> = buttons
            .into_iter()
            .map(|btn| {
                let mut btn_json = json!({
                    "text": btn.text,
                });

                if let Some(url) = btn.url {
                    btn_json["url"] = json!(url);
                }

                if let Some(callback_data) = btn.callback_data {
                    btn_json["callback_data"] = json!(callback_data);
                }

                btn_json
            })
            .collect();

        let payload = json!({
            "chat_id": parts[0],
            "message_id": parts[1],
            "reply_markup": {
                "inline_keyboard": [keyboard]
            }
        });

        self.make_request("editMessageReplyMarkup", &payload)?;
        Ok(())
    }

    async fn delete_message(&self, message_id: &str) -> Result<()> {
        let parts: Vec<&str> = message_id.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid message_id format. Expected 'chat_id:message_id'"));
        }

        let payload = json!({
            "chat_id": parts[0],
            "message_id": parts[1],
        });

        self.make_request("deleteMessage", &payload)?;
        Ok(())
    }

    async fn get_user_info(&self, user_id: &str) -> Result<Option<User>> {
        let payload = json!({
            "chat_id": user_id,
        });

        match self.make_request("getChat", &payload) {
            Ok(response) => {
                let chat = &response["result"];
                if chat["type"] == "private" {
                    Ok(Some(User {
                        id: chat["id"].as_str().unwrap_or("").to_string(),
                        username: chat["username"].as_str().map(|s| s.to_string()),
                        first_name: chat["first_name"].as_str().map(|s| s.to_string()),
                        last_name: chat["last_name"].as_str().map(|s| s.to_string()),
                        display_name: format!(
                            "{} {}",
                            chat["first_name"].as_str().unwrap_or(""),
                            chat["last_name"].as_str().unwrap_or("")
                        ).trim().to_string(),
                        is_bot: chat["is_bot"].as_bool().unwrap_or(false),
                        is_verified: chat["is_verified"].as_bool().unwrap_or(false),
                        is_premium: chat["is_premium"].as_bool().unwrap_or(false),
                        language_code: chat["language_code"].as_str().map(|s| s.to_string()),
                        platform_specific: HashMap::new(),
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    async fn get_chat_info(&self, chat_id: &str) -> Result<Option<Chat>> {
        let payload = json!({
            "chat_id": chat_id,
        });

        match self.make_request("getChat", &payload) {
            Ok(response) => {
                let chat = &response["result"];
                Ok(Some(Chat {
                    id: chat["id"].as_str().unwrap_or("").to_string(),
                    chat_type: match chat["type"].as_str().unwrap_or("") {
                        "private" => ChatType::Private,
                        "group" => ChatType::Group,
                        "supergroup" => ChatType::Supergroup,
                        "channel" => ChatType::Channel,
                        _ => ChatType::Private,
                    },
                    title: chat["title"].as_str().map(|s| s.to_string()),
                    username: chat["username"].as_str().map(|s| s.to_string()),
                    description: chat["description"].as_str().map(|s| s.to_string()),
                    invite_link: chat["invite_link"].as_str().map(|s| s.to_string()),
                    permissions: None, // Would need additional API call
                    member_count: None, // Would need additional API call
                    platform_specific: HashMap::new(),
                }))
            }
            Err(_) => Ok(None),
        }
    }

    async fn start_listening(&self) -> Result<()> {
        // This would typically set up webhooks or long polling
        // For simplicity, we'll just return success
        Ok(())
    }

    async fn stop_listening(&self) -> Result<()> {
        // Stop webhook or long polling
        Ok(())
    }

    fn get_capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            supports_markdown: true,
            supports_html: true,
            supports_inline_buttons: true,
            supports_reply_keyboards: true,
            supports_media: vec![
                MediaType::Photo,
                MediaType::Video,
                MediaType::Audio,
                MediaType::Document,
                MediaType::Voice,
                MediaType::VideoNote,
                MediaType::Animation,
                MediaType::Sticker,
            ],
            supports_location: true,
            supports_contact: true,
            supports_polls: true,
            supports_webhooks: true,
            supports_long_polling: true,
            supports_message_editing: true,
            supports_message_deletion: true,
            max_message_length: Some(4096),
            max_media_size_mb: Some(50),
            rate_limit_per_minute: 30,
        }
    }
}

// Discord adapter implementation (placeholder)
pub struct DiscordAdapter {
    client: Option<ureq::Agent>,
    bot_token: String,
    config: Option<PlatformConfig>,
}

impl DiscordAdapter {
    pub fn new(bot_token: String) -> Self {
        Self {
            client: None,
            bot_token,
            config: None,
        }
    }
}

#[async_trait]
impl MessagingPlatform for DiscordAdapter {
    fn platform_name(&self) -> &str {
        "discord"
    }

    fn platform_version(&self) -> &str {
        "10"
    }

    async fn initialize(&mut self, config: &PlatformConfig) -> Result<()> {
        self.client = Some(ureq::AgentBuilder::new().build());
        self.config = Some(config.clone());
        Ok(())
    }

    async fn is_healthy(&self) -> Result<bool> {
        Ok(self.client.is_some())
    }

    async fn send_message(&self, _recipient: &str, _content: &str) -> Result<()> {
        // Discord API implementation would go here
        Ok(())
    }

    async fn send_message_with_options(
        &self,
        _recipient: &str,
        _content: &str,
        _options: MessageOptions,
    ) -> Result<()> {
        // Discord implementation
        Ok(())
    }

    async fn send_buttons(
        &self,
        _recipient: &str,
        _content: &str,
        _buttons: Vec<InlineButton>,
    ) -> Result<()> {
        // Discord implementation with components
        Ok(())
    }

    async fn send_media(&self, _recipient: &str, _media: MediaContent) -> Result<()> {
        // Discord media implementation
        Ok(())
    }

    async fn edit_message(&self, _message_id: &str, _new_content: &str) -> Result<()> {
        Ok(())
    }

    async fn edit_message_buttons(&self, _message_id: &str, _buttons: Vec<InlineButton>) -> Result<()> {
        Ok(())
    }

    async fn delete_message(&self, _message_id: &str) -> Result<()> {
        Ok(())
    }

    async fn get_user_info(&self, _user_id: &str) -> Result<Option<User>> {
        Ok(None)
    }

    async fn get_chat_info(&self, _chat_id: &str) -> Result<Option<Chat>> {
        Ok(None)
    }

    async fn start_listening(&self) -> Result<()> {
        Ok(())
    }

    async fn stop_listening(&self) -> Result<()> {
        Ok(())
    }

    fn get_capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            supports_markdown: true,
            supports_html: false,
            supports_inline_buttons: true,
            supports_reply_keyboards: false,
            supports_media: vec![
                MediaType::Photo,
                MediaType::Video,
                MediaType::Audio,
                MediaType::Document,
            ],
            supports_location: false,
            supports_contact: false,
            supports_polls: true,
            supports_webhooks: true,
            supports_long_polling: false,
            supports_message_editing: true,
            supports_message_deletion: true,
            max_message_length: Some(2000),
            max_media_size_mb: Some(8),
            rate_limit_per_minute: 50,
        }
    }
}

// Slack adapter implementation (placeholder)
pub struct SlackAdapter {
    client: Option<ureq::Agent>,
    bot_token: String,
    config: Option<PlatformConfig>,
}

impl SlackAdapter {
    pub fn new(bot_token: String) -> Self {
        Self {
            client: None,
            bot_token,
            config: None,
        }
    }
}

#[async_trait]
impl MessagingPlatform for SlackAdapter {
    fn platform_name(&self) -> &str {
        "slack"
    }

    fn platform_version(&self) -> &str {
        "1.0"
    }

    async fn initialize(&mut self, config: &PlatformConfig) -> Result<()> {
        self.client = Some(ureq::AgentBuilder::new().build());
        self.config = Some(config.clone());
        Ok(())
    }

    async fn is_healthy(&self) -> Result<bool> {
        Ok(self.client.is_some())
    }

    async fn send_message(&self, _recipient: &str, _content: &str) -> Result<()> {
        Ok(())
    }

    async fn send_message_with_options(
        &self,
        _recipient: &str,
        _content: &str,
        _options: MessageOptions,
    ) -> Result<()> {
        Ok(())
    }

    async fn send_buttons(
        &self,
        _recipient: &str,
        _content: &str,
        _buttons: Vec<InlineButton>,
    ) -> Result<()> {
        Ok(())
    }

    async fn send_media(&self, _recipient: &str, _media: MediaContent) -> Result<()> {
        Ok(())
    }

    async fn edit_message(&self, _message_id: &str, _new_content: &str) -> Result<()> {
        Ok(())
    }

    async fn edit_message_buttons(&self, _message_id: &str, _buttons: Vec<InlineButton>) -> Result<()> {
        Ok(())
    }

    async fn delete_message(&self, _message_id: &str) -> Result<()> {
        Ok(())
    }

    async fn get_user_info(&self, _user_id: &str) -> Result<Option<User>> {
        Ok(None)
    }

    async fn get_chat_info(&self, _chat_id: &str) -> Result<Option<Chat>> {
        Ok(None)
    }

    async fn start_listening(&self) -> Result<()> {
        Ok(())
    }

    async fn stop_listening(&self) -> Result<()> {
        Ok(())
    }

    fn get_capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            supports_markdown: true,
            supports_html: false,
            supports_inline_buttons: true,
            supports_reply_keyboards: true,
            supports_media: vec![
                MediaType::Photo,
                MediaType::Video,
                MediaType::Audio,
                MediaType::Document,
            ],
            supports_location: false,
            supports_contact: false,
            supports_polls: false,
            supports_webhooks: true,
            supports_long_polling: false,
            supports_message_editing: true,
            supports_message_deletion: true,
            max_message_length: Some(40000),
            max_media_size_mb: Some(1000),
            rate_limit_per_minute: 60,
        }
    }
}
