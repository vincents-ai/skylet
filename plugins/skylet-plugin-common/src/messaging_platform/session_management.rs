// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// User session management for chat platforms
// Provides persistent user sessions, conversation state, and user preferences
use super::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

// Session manager
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, UserSession>>>,
    session_timeout: Arc<RwLock<Duration>>,
    storage: Option<Arc<dyn SessionStorage>>,
    auto_save: Arc<RwLock<bool>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_timeout: Arc::new(RwLock::new(Duration::from_secs(3600))), // 1 hour default
            storage: None,
            auto_save: Arc::new(RwLock::new(true)),
        }
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        let manager = Self::new();
        manager.set_session_timeout(timeout);
        manager
    }

    pub fn with_storage(mut self, storage: Arc<dyn SessionStorage>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn set_session_timeout(&self, timeout: Duration) {
        let mut t = self.session_timeout.write().unwrap();
        *t = timeout;
    }

    pub fn set_auto_save(&self, auto_save: bool) {
        let mut asave = self.auto_save.write().unwrap();
        *asave = auto_save;
    }

    pub async fn get_or_create_session(&self, user_id: &str, platform: &str) -> UserSession {
        let session_key = format!("{}:{}", platform, user_id);

        // Try to get existing session
        {
            let sessions = self.sessions.read().unwrap();
            if let Some(session) = sessions.get(&session_key) {
                // Check if session is still valid
                if !session.is_expired() {
                    return session.clone();
                }
            }
        }

        // Create new session
        let mut sessions = self.sessions.write().unwrap();
        let session = UserSession::new(user_id.to_string(), platform.to_string());
        sessions.insert(session_key.clone(), session.clone());

        // Save to storage if available
        if self.storage.is_some() && *self.auto_save.read().unwrap() {
            if let Err(e) = self.save_session(&session).await {
                eprintln!("Failed to save session: {}", e);
            }
        }

        session
    }

    pub async fn get_session(&self, user_id: &str, platform: &str) -> Option<UserSession> {
        let session_key = format!("{}:{}", platform, user_id);

        let sessions = self.sessions.read().unwrap();
        let session = sessions.get(&session_key)?;

        if session.is_expired() {
            return None;
        }

        Some(session.clone())
    }

    pub async fn update_session(&self, session: &UserSession) -> Result<()> {
        let session_key = format!("{}:{}", session.platform, session.user_id);

        {
            let mut sessions = self.sessions.write().unwrap();
            let mut updated_session = session.clone();
            updated_session.touch(); // Update last activity
            sessions.insert(session_key, updated_session);
        }

        // Save to storage if available
        if self.storage.is_some() && *self.auto_save.read().unwrap() {
            self.save_session(session).await?;
        }

        Ok(())
    }

    pub async fn save_session(&self, session: &UserSession) -> Result<()> {
        if let Some(storage) = &self.storage {
            storage.save_session(session).await?;
        }
        Ok(())
    }

    pub async fn load_session(&self, user_id: &str, platform: &str) -> Result<Option<UserSession>> {
        if let Some(storage) = &self.storage {
            let session = storage.load_session(user_id, platform).await?;

            if let Some(ref s) = session {
                if !s.is_expired() {
                    // Add to memory cache
                    let session_key = format!("{}:{}", platform, user_id);
                    let mut sessions = self.sessions.write().unwrap();
                    sessions.insert(session_key, s.clone());
                }
            }

            Ok(session)
        } else {
            Ok(None)
        }
    }

    pub async fn delete_session(&self, user_id: &str, platform: &str) -> Result<()> {
        let session_key = format!("{}:{}", platform, user_id);

        {
            let mut sessions = self.sessions.write().unwrap();
            sessions.remove(&session_key);
        }

        if let Some(storage) = &self.storage {
            storage.delete_session(user_id, platform).await?;
        }

        Ok(())
    }

    pub async fn cleanup_expired_sessions(&self) -> Result<usize> {
        let mut expired_keys = Vec::new();

        {
            let sessions = self.sessions.read().unwrap();
            for (key, session) in sessions.iter() {
                if session.is_expired() {
                    expired_keys.push(key.clone());
                }
            }
        }

        // Remove expired sessions
        {
            let mut sessions = self.sessions.write().unwrap();
            for key in expired_keys.iter() {
                sessions.remove(key);
            }
        }

        // Also cleanup from storage
        if let Some(storage) = &self.storage {
            storage.cleanup_expired_sessions().await?;
        }

        Ok(expired_keys.len())
    }

    pub async fn get_all_sessions(&self) -> Vec<UserSession> {
        let sessions = self.sessions.read().unwrap();
        sessions.values().cloned().collect()
    }

    pub async fn get_active_sessions_count(&self) -> usize {
        let sessions = self.sessions.read().unwrap();
        sessions.values().filter(|s| !s.is_expired()).count()
    }

    pub async fn set_user_data(
        &self,
        user_id: &str,
        platform: &str,
        key: String,
        value: String,
    ) -> Result<()> {
        let mut session = self.get_or_create_session(user_id, platform).await;
        session.set_data(key, value);
        self.update_session(&session).await
    }

    pub async fn get_user_data(&self, user_id: &str, platform: &str, key: &str) -> Option<String> {
        if let Some(session) = self.get_session(user_id, platform).await {
            session.get_data(key)
        } else {
            None
        }
    }

    pub async fn set_user_preference(
        &self,
        user_id: &str,
        platform: &str,
        preference: UserPreference,
    ) -> Result<()> {
        let mut session = self.get_or_create_session(user_id, platform).await;
        session.set_preference(preference);
        self.update_session(&session).await
    }

    pub async fn get_user_preference(
        &self,
        user_id: &str,
        platform: &str,
        key: &str,
    ) -> Option<UserPreference> {
        if let Some(session) = self.get_session(user_id, platform).await {
            session.get_preference(key)
        } else {
            None
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

// User session structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub session_id: String,
    pub user_id: String,
    pub platform: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub conversation_state: ConversationState,
    pub user_data: HashMap<String, String>,
    pub preferences: HashMap<String, UserPreference>,
    pub message_count: u32,
    pub is_bot: bool,
    pub language_code: Option<String>,
    pub timezone: Option<String>,
}

impl UserSession {
    pub fn new(user_id: String, platform: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            user_id,
            platform,
            created_at: now,
            last_activity: now,
            conversation_state: ConversationState::new(),
            user_data: HashMap::new(),
            preferences: HashMap::new(),
            message_count: 0,
            is_bot: false,
            language_code: None,
            timezone: None,
        }
    }

    /// Check if session is expired using the default 1-hour timeout
    pub fn is_expired(&self) -> bool {
        self.is_expired_with_timeout(Duration::from_secs(3600))
    }

    /// Check if session is expired with a custom timeout
    pub fn is_expired_with_timeout(&self, timeout: Duration) -> bool {
        let elapsed = chrono::Utc::now() - self.last_activity;
        elapsed > chrono::Duration::from_std(timeout).unwrap_or(chrono::Duration::hours(24))
    }

    pub fn touch(&mut self) {
        self.last_activity = chrono::Utc::now();
    }

    pub fn increment_message_count(&mut self) {
        self.message_count += 1;
        self.touch();
    }

    pub fn set_data(&mut self, key: String, value: String) {
        self.user_data.insert(key, value);
        self.touch();
    }

    pub fn get_data(&self, key: &str) -> Option<String> {
        self.user_data.get(key).cloned()
    }

    pub fn set_preference(&mut self, preference: UserPreference) {
        self.preferences.insert(preference.key.clone(), preference);
        self.touch();
    }

    pub fn get_preference(&self, key: &str) -> Option<UserPreference> {
        self.preferences.get(key).cloned()
    }

    pub fn update_conversation_state(&mut self, state: ConversationState) {
        self.conversation_state = state;
        self.touch();
    }

    pub fn get_conversation_context(&self) -> ConversationContext {
        ConversationContext {
            user_id: self.user_id.clone(),
            platform: self.platform.clone(),
            session_id: self.session_id.clone(),
            user_data: self.user_data.clone(),
            preferences: self.preferences.clone(),
            message_count: self.message_count,
            conversation_state: self.conversation_state.clone(),
            language_code: self.language_code.clone(),
            timezone: self.timezone.clone(),
        }
    }
}

// Conversation state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationState {
    pub current_step: String,
    pub step_data: HashMap<String, String>,
    pub waiting_for_input: bool,
    pub input_type: Option<String>,
    pub timeout_at: Option<chrono::DateTime<chrono::Utc>>,
    pub context_stack: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl ConversationState {
    pub fn new() -> Self {
        Self {
            current_step: "start".to_string(),
            step_data: HashMap::new(),
            waiting_for_input: false,
            input_type: None,
            timeout_at: None,
            context_stack: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn push_context(&mut self, context: String) {
        self.context_stack.push(context);
    }

    pub fn pop_context(&mut self) -> Option<String> {
        self.context_stack.pop()
    }

    pub fn peek_context(&self) -> Option<&String> {
        self.context_stack.last()
    }

    pub fn set_step(&mut self, step: String) {
        self.current_step = step;
    }

    pub fn set_step_data(&mut self, key: String, value: String) {
        self.step_data.insert(key, value);
    }

    pub fn get_step_data(&self, key: &str) -> Option<&String> {
        self.step_data.get(key)
    }

    pub fn start_waiting(&mut self, input_type: String, timeout_seconds: u64) {
        self.waiting_for_input = true;
        self.input_type = Some(input_type);
        self.timeout_at =
            Some(chrono::Utc::now() + chrono::Duration::seconds(timeout_seconds as i64));
    }

    pub fn stop_waiting(&mut self) {
        self.waiting_for_input = false;
        self.input_type = None;
        self.timeout_at = None;
    }

    pub fn is_waiting(&self) -> bool {
        if let Some(timeout) = self.timeout_at {
            self.waiting_for_input && chrono::Utc::now() < timeout
        } else {
            self.waiting_for_input
        }
    }
}

impl Default for ConversationState {
    fn default() -> Self {
        Self::new()
    }
}

// User preference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreference {
    pub key: String,
    pub value: PreferenceValue,
    pub category: String,
    pub description: Option<String>,
    pub is_private: bool,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreferenceValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Json(serde_json::Value),
}

impl PreferenceValue {
    pub fn as_string(&self) -> Option<&String> {
        match self {
            PreferenceValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            PreferenceValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            PreferenceValue::Float(f) => Some(*f),
            PreferenceValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            PreferenceValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            PreferenceValue::Json(j) => Some(j),
            _ => None,
        }
    }
}

// Conversation context for commands
#[derive(Debug, Clone)]
pub struct ConversationContext {
    pub user_id: String,
    pub platform: String,
    pub session_id: String,
    pub user_data: HashMap<String, String>,
    pub preferences: HashMap<String, UserPreference>,
    pub message_count: u32,
    pub conversation_state: ConversationState,
    pub language_code: Option<String>,
    pub timezone: Option<String>,
}

impl ConversationContext {
    pub fn get_preference_value(&self, key: &str) -> Option<PreferenceValue> {
        self.preferences.get(key).map(|p| p.value.clone())
    }

    pub fn get_string_preference(&self, key: &str) -> Option<String> {
        self.get_preference_value(key)?.as_string().cloned()
    }

    pub fn get_integer_preference(&self, key: &str) -> Option<i64> {
        self.get_preference_value(key)?.as_integer()
    }

    pub fn get_float_preference(&self, key: &str) -> Option<f64> {
        self.get_preference_value(key)?.as_float()
    }

    pub fn get_boolean_preference(&self, key: &str) -> Option<bool> {
        self.get_preference_value(key)?.as_boolean()
    }

    pub fn get_user_data(&self, key: &str) -> Option<&String> {
        self.user_data.get(key)
    }

    pub fn is_new_user(&self) -> bool {
        self.message_count <= 1
    }

    pub fn is_in_conversation(&self) -> bool {
        self.conversation_state.is_waiting()
    }
}

// Session storage trait for persistence
#[async_trait]
pub trait SessionStorage: Send + Sync {
    async fn save_session(&self, session: &UserSession) -> Result<()>;
    async fn load_session(&self, user_id: &str, platform: &str) -> Result<Option<UserSession>>;
    async fn delete_session(&self, user_id: &str, platform: &str) -> Result<()>;
    async fn cleanup_expired_sessions(&self) -> Result<usize>;
    async fn get_all_sessions(&self) -> Result<Vec<UserSession>>;
}

// In-memory session storage (for testing or simple cases)
pub struct InMemorySessionStorage {
    sessions: Arc<RwLock<HashMap<String, UserSession>>>,
}

impl InMemorySessionStorage {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn get_session_key(user_id: &str, platform: &str) -> String {
        format!("{}:{}", platform, user_id)
    }
}

impl Default for InMemorySessionStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionStorage for InMemorySessionStorage {
    async fn save_session(&self, session: &UserSession) -> Result<()> {
        let key = Self::get_session_key(&session.user_id, &session.platform);
        let mut sessions = self.sessions.write().unwrap();
        sessions.insert(key, session.clone());
        Ok(())
    }

    async fn load_session(&self, user_id: &str, platform: &str) -> Result<Option<UserSession>> {
        let key = Self::get_session_key(user_id, platform);
        let sessions = self.sessions.read().unwrap();
        Ok(sessions.get(&key).cloned())
    }

    async fn delete_session(&self, user_id: &str, platform: &str) -> Result<()> {
        let key = Self::get_session_key(user_id, platform);
        let mut sessions = self.sessions.write().unwrap();
        sessions.remove(&key);
        Ok(())
    }

    async fn cleanup_expired_sessions(&self) -> Result<usize> {
        let mut expired_keys = Vec::new();
        let timeout = Duration::from_secs(3600); // 1 hour default

        {
            let sessions = self.sessions.read().unwrap();
            for (key, session) in sessions.iter() {
                if session.is_expired_with_timeout(timeout) {
                    expired_keys.push(key.clone());
                }
            }
        }

        {
            let mut sessions = self.sessions.write().unwrap();
            for key in expired_keys.iter() {
                sessions.remove(key);
            }
        }

        Ok(expired_keys.len())
    }

    async fn get_all_sessions(&self) -> Result<Vec<UserSession>> {
        let sessions = self.sessions.read().unwrap();
        Ok(sessions.values().cloned().collect())
    }
}

// Utility functions
pub fn create_user_session(user_id: &str, platform: &str) -> UserSession {
    UserSession::new(user_id.to_string(), platform.to_string())
}

pub fn create_preference(key: String, value: PreferenceValue, category: String) -> UserPreference {
    UserPreference {
        key,
        value,
        category,
        description: None,
        is_private: false,
        updated_at: chrono::Utc::now(),
    }
}

pub fn create_private_preference(
    key: String,
    value: PreferenceValue,
    category: String,
) -> UserPreference {
    let mut pref = create_preference(key, value, category);
    pref.is_private = true;
    pref
}

// Common preference keys
pub mod preferences {
    pub const LANGUAGE: &str = "language";
    pub const TIMEZONE: &str = "timezone";
    pub const NOTIFICATIONS_ENABLED: &str = "notifications_enabled";
    pub const THEME: &str = "theme";
    pub const AUTO_RESPONSES: &str = "auto_responses";
    pub const DATA_SHARING: &str = "data_sharing";
    pub const PRIVACY_LEVEL: &str = "privacy_level";
}

// Conversation flow helpers
pub struct ConversationFlow {
    steps: Vec<ConversationStep>,
    current_step_index: usize,
}

impl ConversationFlow {
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            current_step_index: 0,
        }
    }

    pub fn add_step(&mut self, step: ConversationStep) {
        self.steps.push(step);
    }

    pub fn get_current_step(&self) -> Option<&ConversationStep> {
        self.steps.get(self.current_step_index)
    }

    pub fn advance_step(&mut self) -> Result<()> {
        if self.current_step_index < self.steps.len() - 1 {
            self.current_step_index += 1;
            Ok(())
        } else {
            Err(anyhow::anyhow!("No more steps in conversation flow"))
        }
    }

    pub fn reset(&mut self) {
        self.current_step_index = 0;
    }

    pub fn is_completed(&self) -> bool {
        self.current_step_index >= self.steps.len()
    }
}

#[derive(Debug, Clone)]
pub struct ConversationStep {
    pub id: String,
    pub prompt: String,
    pub input_type: String,
    pub validator: Option<String>,
    pub timeout_seconds: u64,
    pub max_attempts: u32,
    pub context: HashMap<String, String>,
}
