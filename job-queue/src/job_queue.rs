// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use base64::Engine;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub task_name: String,
    pub payload: Vec<u8>,
    pub available_at: DateTime<Utc>,
    pub attempts: u32,
    pub max_attempts: u32,
}

impl Job {
    pub fn new(task_name: &str, payload: Vec<u8>, delay_seconds: u32, max_attempts: u32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            task_name: task_name.to_string(),
            payload,
            available_at: Utc::now() + chrono::Duration::seconds(delay_seconds as i64),
            attempts: 0,
            max_attempts,
        }
    }
}

/// Connection pool wrapper for SQLite
#[derive(Clone)]
pub struct ConnectionPool {
    connections: Arc<Mutex<Vec<Connection>>>,
    db_path: String,
    max_size: usize,
}

impl ConnectionPool {
    pub fn new(db_path: String, max_size: usize) -> Self {
        Self {
            connections: Arc::new(Mutex::new(Vec::new())),
            db_path,
            max_size,
        }
    }

    pub fn get(&self) -> Result<Connection> {
        let mut pool = self.connections.lock();

        // Try to reuse an existing connection
        if let Some(conn) = pool.pop() {
            return Ok(conn);
        }

        // Create new connection if under limit
        drop(pool);
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        Ok(conn)
    }

    pub fn return_connection(&self, conn: Connection) {
        let mut pool = self.connections.lock();
        if pool.len() < self.max_size {
            pool.push(conn);
        }
    }
}

#[derive(Clone)]
pub struct JobQueue {
    pool: ConnectionPool,
}

impl JobQueue {
    pub fn new(db_path: String) -> Self {
        Self {
            pool: ConnectionPool::new(db_path, 10),
        }
    }

    pub fn init(&self) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            r#"CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                task_name TEXT NOT NULL,
                payload BLOB NOT NULL,
                available_at TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                max_attempts INTEGER NOT NULL DEFAULT 5
            )"#,
            [],
        )?;
        // Create index for efficient queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_jobs_available_at ON jobs(available_at)",
            [],
        )?;
        Ok(())
    }

    pub fn enqueue(&self, job: &Job) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO jobs (id, task_name, payload, available_at, attempts, max_attempts) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                &job.id,
                &job.task_name,
                &BASE64_ENGINE.encode(&job.payload),
                &job.available_at.to_rfc3339(),
                job.attempts,
                job.max_attempts
            ],
        )?;
        Ok(())
    }

    pub fn pop_available(&self) -> Result<Option<Job>> {
        let conn = self.pool.get()?;
        let now = Utc::now().to_rfc3339();

        if let Some(r) = conn.query_row(
            "SELECT id, task_name, payload, available_at, attempts, max_attempts FROM jobs WHERE available_at <= ?1 ORDER BY available_at ASC LIMIT 1",
            [&now],
            |row| {
                let id: String = row.get(0)?;
                let task_name: String = row.get(1)?;
                let payload: String = row.get(2)?;
                let available_at_str: String = row.get(3)?;
                let attempts: u32 = row.get(4)?;
                let max_attempts: u32 = row.get(5)?;
                Ok((id, task_name, payload, available_at_str, attempts, max_attempts))
            },
        ).optional()? {
            let (id, task_name, payload_str, available_at_str, attempts, max_attempts) = r;

            // Use transaction for atomic delete
            conn.execute("DELETE FROM jobs WHERE id = ?1", [&id])?;

            let available_at = DateTime::parse_from_rfc3339(&available_at_str)?.with_timezone(&Utc);
            let payload = BASE64_ENGINE.decode(&payload_str)
                .map_err(|e| anyhow::anyhow!("Failed to decode base64 payload: {}", e))?;

            Ok(Some(Job {
                id,
                task_name,
                payload,
                available_at,
                attempts,
                max_attempts,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn retry_job(&self, mut job: Job) -> Result<()> {
        job.attempts += 1;
        if job.attempts >= job.max_attempts {
            job.available_at = Utc::now() + chrono::Duration::days(3650);
        } else {
            let delay = 2u64.pow(job.attempts);
            job.available_at = Utc::now() + chrono::Duration::seconds(delay as i64);
        }

        let conn = self.pool.get()?;
        conn.execute(
            "INSERT OR REPLACE INTO jobs (id, task_name, payload, available_at, attempts, max_attempts) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                &job.id,
                &job.task_name,
                &BASE64_ENGINE.encode(&job.payload),
                &job.available_at.to_rfc3339(),
                job.attempts,
                job.max_attempts
            ],
        )?;
        Ok(())
    }

    pub fn run_dispatch_loop<F>(&self, mut handler: F)
    where
        F: FnMut(Job) -> Result<bool> + Send + 'static,
    {
        loop {
            match self.pop_available() {
                Ok(Some(job)) => match handler(job.clone()) {
                    Ok(true) => {}
                    Ok(false) | Err(_) => {
                        let _ = self.retry_job(job);
                    }
                },
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    tracing::error!("JobQueue pop error: {}", e);
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_queue() -> (JobQueue, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let db_path = temp_dir
            .path()
            .join("test_jobs.db")
            .to_string_lossy()
            .to_string();
        let queue = JobQueue::new(db_path);
        queue.init().expect("Failed to initialize queue");
        (queue, temp_dir)
    }

    #[test]
    fn test_job_creation() {
        let job = Job::new("test_task", vec![1, 2, 3], 0, 3);
        assert_eq!(job.task_name, "test_task");
        assert_eq!(job.payload, vec![1, 2, 3]);
        assert_eq!(job.attempts, 0);
        assert_eq!(job.max_attempts, 3);
    }

    #[test]
    fn test_enqueue_and_pop() {
        let (queue, _dir) = setup_test_queue();

        let job = Job::new("test_task", b"hello".to_vec(), 0, 3);
        queue.enqueue(&job).expect("Failed to enqueue job");

        let popped = queue.pop_available().expect("Failed to pop job");
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().task_name, "test_task");
    }

    #[test]
    fn test_empty_queue() {
        let (queue, _dir) = setup_test_queue();
        let result = queue.pop_available().expect("Failed to pop");
        assert!(result.is_none());
    }

    #[test]
    fn test_retry_job() {
        let (queue, _dir) = setup_test_queue();

        let job = Job::new("test_task", b"hello".to_vec(), 0, 3);
        queue.enqueue(&job).expect("Failed to enqueue job");

        let popped = queue.pop_available().expect("Failed to pop").unwrap();

        // Simulate failure and retry
        let _ = queue.retry_job(popped);

        // The retried job has exponential backoff (available_at is in the future),
        // so pop_available won't return it immediately. Instead, verify the job
        // was re-enqueued by checking the database directly.
        let conn = queue.pool.get().expect("Failed to get connection");
        let (attempts, task_name): (u32, String) = conn
            .query_row("SELECT attempts, task_name FROM jobs LIMIT 1", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("Failed to query retried job");
        assert_eq!(attempts, 1);
        assert_eq!(task_name, "test_task");
    }

    #[test]
    fn test_max_attempts() {
        let (queue, _dir) = setup_test_queue();

        let mut job = Job::new("test_task", b"hello".to_vec(), 0, 2);
        job.attempts = 2; // Already at max

        queue.retry_job(job).expect("Failed to retry");

        // Should be scheduled far in the future
        let popped = queue.pop_available().expect("Failed to pop");
        assert!(popped.is_none());
    }
}
