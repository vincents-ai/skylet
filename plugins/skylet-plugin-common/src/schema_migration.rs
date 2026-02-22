// Schema migration system for database plugins
// Provides version-controlled database schema management with rollback support
use crate::database::{DatabaseConnection, DatabaseError, DatabaseRow, DatabaseValue, ToSql};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};

// Migration result and status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    pub migration_id: String,
    pub version: i64,
    pub name: String,
    pub executed_at: DateTime<Utc>,
    pub execution_time_ms: u64,
    pub success: bool,
    pub error_message: Option<String>,
}

impl MigrationResult {
    pub fn success(migration_id: String, version: i64, name: String, execution_time_ms: u64) -> Self {
        Self {
            migration_id,
            version,
            name,
            executed_at: Utc::now(),
            execution_time_ms,
            success: true,
            error_message: None,
        }
    }

    pub fn failure(migration_id: String, version: i64, name: String, error: String) -> Self {
        Self {
            migration_id,
            version,
            name,
            executed_at: Utc::now(),
            execution_time_ms: 0,
            success: false,
            error_message: Some(error),
        }
    }
}

// Migration definition
#[derive(Debug, Clone)]
pub struct Migration {
    pub id: String,
    pub version: i64,
    pub name: String,
    pub description: Option<String>,
    pub up_sql: String,
    pub down_sql: String,
    pub dependencies: Vec<String>,
    pub checksum: String,
}

impl Migration {
    pub fn new(
        id: String,
        version: i64,
        name: String,
        up_sql: String,
        down_sql: String,
    ) -> Self {
        let checksum = Self::calculate_checksum(&up_sql, &down_sql);
        Self {
            id,
            version,
            name,
            description: None,
            up_sql,
            down_sql,
            dependencies: Vec::new(),
            checksum,
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<String>) -> Self {
        self.dependencies = dependencies;
        self
    }

    fn calculate_checksum(up_sql: &str, down_sql: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(up_sql.as_bytes());
        hasher.update(down_sql.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn verify_checksum(&self) -> bool {
        let calculated_checksum = Self::calculate_checksum(&self.up_sql, &self.down_sql);
        calculated_checksum == self.checksum
    }
}

// Migration trait for custom migration logic
#[async_trait]
pub trait MigrationStep: Send + Sync {
    fn id(&self) -> &str;
    fn version(&self) -> i64;
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str> {
        None
    }
    fn dependencies(&self) -> Vec<String> {
        Vec::new()
    }
    async fn up(&self, connection: &dyn DatabaseConnection) -> Result<(), MigrationError>;
    async fn down(&self, connection: &dyn DatabaseConnection) -> Result<(), MigrationError>;
    fn checksum(&self) -> String;
}

// Migration errors
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Migration not found: {id}")]
    NotFound { id: String },

    #[error("Migration {id} already applied")]
    AlreadyApplied { id: String },

    #[error("Migration {id} has unmet dependencies: {dependencies:?}")]
    UnmetDependencies { id: String, dependencies: Vec<String> },

    #[error("Checksum mismatch for migration {id}")]
    ChecksumMismatch { id: String },

    #[error("Cannot rollback: No migrations to rollback")]
    NothingToRollback,

    #[error("Invalid migration state: {0}")]
    InvalidState(String),

    #[error("Migration {id} failed: {error}")]
    MigrationFailed { id: String, error: String },

    #[error("Migration table not found or not initialized")]
    MigrationTableNotInitialized,
}

// Migration manager
pub struct MigrationManager {
    connection: Arc<dyn DatabaseConnection>,
    table_name: String,
    migrations: Vec<Migration>,
    custom_migrations: Vec<Box<dyn MigrationStep>>,
}

impl MigrationManager {
    pub fn new(connection: Arc<dyn DatabaseConnection>) -> Self {
        Self {
            connection,
            table_name: "schema_migrations".to_string(),
            migrations: Vec::new(),
            custom_migrations: Vec::new(),
        }
    }

    pub fn with_table_name(mut self, table_name: String) -> Self {
        self.table_name = table_name;
        self
    }

    pub fn add_migration(&mut self, migration: Migration) {
        self.migrations.push(migration);
    }

    pub fn add_custom_migration(&mut self, migration: Box<dyn MigrationStep>) {
        self.custom_migrations.push(migration);
    }

    pub fn add_migrations(&mut self, migrations: Vec<Migration>) {
        self.migrations.extend(migrations);
    }

    /// Initialize the migration tracking table
    pub async fn initialize(&self) -> Result<(), MigrationError> {
        let create_table_sql = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {} (
                id VARCHAR(255) PRIMARY KEY,
                version BIGINT NOT NULL,
                name VARCHAR(255) NOT NULL,
                description TEXT,
                executed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                execution_time_ms BIGINT NOT NULL,
                checksum VARCHAR(64) NOT NULL,
                success BOOLEAN NOT NULL,
                error_message TEXT
            )
            "#,
            self.table_name
        );

        self.connection.execute(&create_table_sql, &[])?;
        
        // Create index for faster lookups
        let create_index_sql = format!(
            "CREATE INDEX IF NOT EXISTS idx_{}_version ON {} (version)",
            self.table_name, self.table_name
        );
        self.connection.execute(&create_index_sql, &[])?;

        Ok(())
    }

    /// Check if migration table exists
    pub async fn is_initialized(&self) -> Result<bool, MigrationError> {
        let check_sql = format!(
            "SELECT COUNT(*) as count FROM information_schema.tables 
             WHERE table_name = '{}'",
            self.table_name
        );

        match self.connection.query_one(&check_sql, &[])? {
            Some(row) => Ok(row.get_i64("count").unwrap_or(0) > 0),
            None => Ok(false),
        }
    }

    /// Get all applied migrations
    pub async fn get_applied_migrations(&self) -> Result<Vec<MigrationResult>, MigrationError> {
        let select_sql = format!(
            "SELECT id, version, name, executed_at, execution_time_ms, success, error_message, checksum 
             FROM {} ORDER BY version",
            self.table_name
        );

        let rows = self.connection.query(&select_sql, &[])?;
        let mut results = Vec::new();

        for row in rows {
            results.push(MigrationResult {
                migration_id: row.get_string("id").unwrap_or_default().clone(),
                version: row.get_i64("version").unwrap_or(0),
                name: row.get_string("name").unwrap_or_default().clone(),
                executed_at: Utc::now(), // Would parse from actual timestamp
                execution_time_ms: row.get_i64("execution_time_ms").unwrap_or(0) as u64,
                success: row.get_bool("success").unwrap_or(true),
                error_message: row.get_string("error_message").cloned(),
            });
        }

        Ok(results)
    }

    /// Get the latest applied migration version
    pub async fn get_current_version(&self) -> Result<Option<i64>, MigrationError> {
        let select_sql = format!(
            "SELECT MAX(version) as max_version FROM {} WHERE success = true",
            self.table_name
        );

        match self.connection.query_one(&select_sql, &[])? {
            Some(row) => Ok(row.get_i64("max_version")),
            None => Ok(None),
        }
    }

    /// Check if a specific migration has been applied
    pub async fn is_migration_applied(&self, migration_id: &str) -> Result<bool, MigrationError> {
        let select_sql = format!(
            "SELECT COUNT(*) as count FROM {} WHERE id = ? AND success = true",
            self.table_name
        );

        match self.connection.query_one(&select_sql, &[&migration_id.to_sql()])? {
            Some(row) => Ok(row.get_i64("count").unwrap_or(0) > 0),
            None => Ok(false),
        }
    }

    /// Get migrations that need to be applied
    pub async fn get_pending_migrations(&self) -> Result<Vec<&Migration>, MigrationError> {
        let applied = self.get_applied_migrations().await?;
        let applied_ids: std::collections::HashSet<&str> = applied
            .iter()
            .filter(|m| m.success)
            .map(|m| m.migration_id.as_str())
            .collect();

        let mut pending = Vec::new();
        
        for migration in &self.migrations {
            if !applied_ids.contains(migration.id.as_str()) {
                // Check dependencies
                if self.check_dependencies_met(migration, &applied_ids) {
                    pending.push(migration);
                }
            }
        }

        // Sort by version
        pending.sort_by_key(|m| m.version);
        Ok(pending)
    }

    fn check_dependencies_met(&self, migration: &Migration, applied_ids: &std::collections::HashSet<&str>) -> bool {
        migration.dependencies.iter().all(|dep| applied_ids.contains(dep.as_str()))
    }

    /// Apply pending migrations
    pub async fn migrate(&self) -> Result<Vec<MigrationResult>, MigrationError> {
        let pending = self.get_pending_migrations().await?;
        let mut results = Vec::new();

        for migration in pending {
            let start_time = std::time::Instant::now();
            
            match self.apply_migration(migration).await {
                Ok(_) => {
                    let execution_time = start_time.elapsed().as_millis() as u64;
                    let result = MigrationResult::success(
                        migration.id.clone(),
                        migration.version,
                        migration.name.clone(),
                        execution_time,
                    );
                    results.push(result);
                }
                Err(e) => {
                    let result = MigrationResult::failure(
                        migration.id.clone(),
                        migration.version,
                        migration.name.clone(),
                        e.to_string(),
                    );
                    results.push(result);
                    return Err(e);
                }
            }
        }

        // Apply custom migrations
        for custom_migration in &self.custom_migrations {
            let start_time = std::time::Instant::now();
            
            match self.apply_custom_migration(custom_migration.as_ref()).await {
                Ok(_) => {
                    let execution_time = start_time.elapsed().as_millis() as u64;
                    let result = MigrationResult::success(
                        custom_migration.id().to_string(),
                        custom_migration.version(),
                        custom_migration.name().to_string(),
                        execution_time,
                    );
                    results.push(result);
                }
                Err(e) => {
                    let result = MigrationResult::failure(
                        custom_migration.id().to_string(),
                        custom_migration.version(),
                        custom_migration.name().to_string(),
                        e.to_string(),
                    );
                    results.push(result);
                    return Err(e);
                }
            }
        }

        Ok(results)
    }

    /// Apply a single migration
    async fn apply_migration(&self, migration: &Migration) -> Result<(), MigrationError> {
        // Verify checksum
        if !migration.verify_checksum() {
            return Err(MigrationError::ChecksumMismatch { id: migration.id.clone() });
        }

        // Check if already applied
        if self.is_migration_applied(&migration.id).await? {
            return Err(MigrationError::AlreadyApplied { id: migration.id.clone() });
        }

        // Start transaction
        self.connection.transaction(|tx| {
            // Execute migration SQL
            tx.execute(&migration.up_sql, &[])?;

            // Record migration
            let insert_sql = format!(
                r#"
                INSERT INTO {} (id, version, name, description, executed_at, execution_time_ms, checksum, success)
                VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, 0, ?, true)
                "#,
                self.table_name
            );

            let params: Vec<&dyn ToSql> = vec![
                &migration.id.to_sql(),
                &migration.version.to_sql(),
                &migration.name.to_sql(),
                &migration.description.to_sql(),
                &migration.checksum.to_sql(),
            ];

            tx.execute(&insert_sql, &params)?;

            Ok(())
        })?;

        Ok(())
    }

    /// Apply a custom migration step
    async fn apply_custom_migration(&self, migration: &dyn MigrationStep) -> Result<(), MigrationError> {
        // Check if already applied
        if self.is_migration_applied(migration.id()).await? {
            return Err(MigrationError::AlreadyApplied { id: migration.id().to_string() });
        }

        // Start transaction
        self.connection.transaction(|tx| {
            // Execute custom migration logic
            let temp_conn = tx as &dyn DatabaseConnection;
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(migration.up(temp_conn))
            })?;

            // Record migration
            let insert_sql = format!(
                r#"
                INSERT INTO {} (id, version, name, description, executed_at, execution_time_ms, checksum, success)
                VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, 0, ?, true)
                "#,
                self.table_name
            );

            let params: Vec<&dyn ToSql> = vec![
                &migration.id().to_sql(),
                &migration.version().to_sql(),
                &migration.name().to_sql(),
                &migration.description().unwrap_or("").to_sql(),
                &migration.checksum().to_sql(),
            ];

            tx.execute(&insert_sql, &params)?;

            Ok(())
        })?;

        Ok(())
    }

    /// Rollback the last migration
    pub async fn rollback(&self, steps: usize) -> Result<Vec<MigrationResult>, MigrationError> {
        let applied = self.get_applied_migrations().await?;
        if applied.is_empty() {
            return Err(MigrationError::NothingToRollback);
        }

        let mut results = Vec::new();
        let steps_to_rollback = steps.min(applied.len());

        for i in 0..steps_to_rollback {
            let migration_result = &applied[applied.len() - 1 - i];
            
            // Find the migration definition
            let migration = self.migrations.iter()
                .find(|m| m.id == migration_result.migration_id)
                .or_else(|| {
                    // Check custom migrations
                    self.custom_migrations.iter()
                        .find(|cm| cm.id() == migration_result.migration_id)
                        .map(|cm| cm as &dyn MigrationStep)
                });

            match migration {
                Some(mig) => {
                    let start_time = std::time::Instant::now();
                    
                    match self.rollback_migration(mig, migration_result).await {
                        Ok(_) => {
                            let execution_time = start_time.elapsed().as_millis() as u64;
                            let result = MigrationResult::success(
                                migration_result.migration_id.clone(),
                                migration_result.version,
                                format!("ROLLBACK: {}", migration_result.name),
                                execution_time,
                            );
                            results.push(result);
                        }
                        Err(e) => {
                            let result = MigrationResult::failure(
                                migration_result.migration_id.clone(),
                                migration_result.version,
                                format!("ROLLBACK: {}", migration_result.name),
                                e.to_string(),
                            );
                            results.push(result);
                            return Err(e);
                        }
                    }
                }
                None => {
                    return Err(MigrationError::NotFound { 
                        id: migration_result.migration_id.clone() 
                    });
                }
            }
        }

        Ok(results)
    }

    /// Rollback a specific migration
    async fn rollback_migration(
        &self,
        migration: &dyn MigrationStep,
        applied_record: &MigrationResult,
    ) -> Result<(), MigrationError> {
        self.connection.transaction(|tx| {
            // Execute rollback SQL or custom logic
            if let Some(sql_migration) = self.migrations.iter()
                .find(|m| m.id == applied_record.migration_id) {
                tx.execute(&sql_migration.down_sql, &[])?;
            } else {
                // Custom migration
                let temp_conn = tx as &dyn DatabaseConnection;
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(migration.down(temp_conn))
                })?;
            }

            // Remove migration record
            let delete_sql = format!("DELETE FROM {} WHERE id = ?", self.table_name);
            tx.execute(&delete_sql, &[&applied_record.migration_id.to_sql()])?;

            Ok(())
        })?;

        Ok(())
    }

    /// Get migration status
    pub async fn get_status(&self) -> Result<MigrationStatus, MigrationError> {
        if !self.is_initialized().await? {
            return Ok(MigrationStatus::NotInitialized);
        }

        let applied = self.get_applied_migrations().await?;
        let pending = self.get_pending_migrations().await?;
        let current_version = self.get_current_version().await?;

        Ok(MigrationStatus::Ready {
            current_version,
            applied_count: applied.len(),
            pending_count: pending.len(),
            last_migration: applied.last().cloned(),
        })
    }

    /// Validate migration integrity
    pub async fn validate(&self) -> Result<Vec<MigrationValidationResult>, MigrationError> {
        let applied = self.get_applied_migrations().await?;
        let mut results = Vec::new();

        for applied_record in applied {
            if !applied_record.success {
                results.push(MigrationValidationResult {
                    migration_id: applied_record.migration_id.clone(),
                    status: ValidationStatus::Failed,
                    message: applied_record.error_message.clone(),
                });
                continue;
            }

            // Find the migration definition
            if let Some(migration) = self.migrations.iter()
                .find(|m| m.id == applied_record.migration_id) {
                
                if !migration.verify_checksum() {
                    results.push(MigrationValidationResult {
                        migration_id: applied_record.migration_id.clone(),
                        status: ValidationStatus::ChecksumMismatch,
                        message: "Migration file has been modified since application".to_string(),
                    });
                } else {
                    results.push(MigrationValidationResult {
                        migration_id: applied_record.migration_id.clone(),
                        status: ValidationStatus::Valid,
                        message: None,
                    });
                }
            } else {
                results.push(MigrationValidationResult {
                    migration_id: applied_record.migration_id.clone(),
                    status: ValidationStatus::Missing,
                    message: "Migration definition not found".to_string(),
                });
            }
        }

        Ok(results)
    }
}

// Migration status
#[derive(Debug, Clone)]
pub enum MigrationStatus {
    NotInitialized,
    Ready {
        current_version: Option<i64>,
        applied_count: usize,
        pending_count: usize,
        last_migration: Option<MigrationResult>,
    },
}

// Migration validation result
#[derive(Debug, Clone)]
pub struct MigrationValidationResult {
    pub migration_id: String,
    pub status: ValidationStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationStatus {
    Valid,
    Failed,
    ChecksumMismatch,
    Missing,
    UnmetDependencies,
}

// Migration builder for easy migration creation
pub struct MigrationBuilder {
    id: String,
    version: i64,
    name: String,
    description: Option<String>,
    up_sql: String,
    down_sql: String,
    dependencies: Vec<String>,
}

impl MigrationBuilder {
    pub fn new(id: String, version: i64, name: String) -> Self {
        Self {
            id,
            version,
            name,
            description: None,
            up_sql: String::new(),
            down_sql: String::new(),
            dependencies: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn up(mut self, sql: impl Into<String>) -> Self {
        self.up_sql = sql.into();
        self
    }

    pub fn down(mut self, sql: impl Into<String>) -> Self {
        self.down_sql = sql.into();
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<String>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn add_dependency(mut self, dependency: String) -> Self {
        self.dependencies.push(dependency);
        self
    }

    pub fn build(self) -> Migration {
        let mut migration = Migration::new(
            self.id,
            self.version,
            self.name,
            self.up_sql,
            self.down_sql,
        );

        if let Some(description) = self.description {
            migration = migration.with_description(description);
        }

        if !self.dependencies.is_empty() {
            migration = migration.with_dependencies(self.dependencies);
        }

        migration
    }
}

// Utility functions
pub fn create_migration(
    id: String,
    version: i64,
    name: String,
    up_sql: String,
    down_sql: String,
) -> Migration {
    Migration::new(id, version, name, up_sql, down_sql)
}

pub fn migration_builder(id: String, version: i64, name: String) -> MigrationBuilder {
    MigrationBuilder::new(id, version, name)
}

// Common migration templates
pub mod templates {
    pub fn create_table_migration(
        id: String,
        version: i64,
        name: String,
        table_name: &str,
        columns: &str,
    ) -> Migration {
        let up_sql = format!("CREATE TABLE {} ({});", table_name, columns);
        let down_sql = format!("DROP TABLE {};", table_name);
        Migration::new(id, version, name, up_sql, down_sql)
    }

    pub fn add_column_migration(
        id: String,
        version: i64,
        name: String,
        table_name: &str,
        column_def: &str,
    ) -> Migration {
        let up_sql = format!("ALTER TABLE {} ADD COLUMN {};", table_name, column_def);
        let down_sql = format!("ALTER TABLE {} DROP COLUMN {};", table_name, 
                              column_def.split_whitespace().next().unwrap_or(""));
        Migration::new(id, version, name, up_sql, down_sql)
    }

    pub fn add_index_migration(
        id: String,
        version: i64,
        name: String,
        index_name: &str,
        table_name: &str,
        columns: &str,
        unique: bool,
    ) -> Migration {
        let unique_str = if unique { "UNIQUE " } else { "" };
        let up_sql = format!(
            "CREATE {}INDEX {} ON {} ({});", 
            unique_str, index_name, table_name, columns
        );
        let down_sql = format!("DROP INDEX {};", index_name);
        Migration::new(id, version, name, up_sql, down_sql)
    }
}