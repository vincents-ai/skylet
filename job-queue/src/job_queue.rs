// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use base64::Engine;
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
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

#[derive(Clone)]
pub struct JobQueue {
    db_path: String,
}

impl JobQueue {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

    pub fn init(&self) -> Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
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
        Ok(())
    }

    pub fn enqueue(&self, job: &Job) -> Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO jobs (id, task_name, payload, available_at, attempts, max_attempts) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            [&job.id, &job.task_name, &BASE64_ENGINE.encode(&job.payload), &job.available_at.to_rfc3339(), &(job.attempts.to_string()), &(job.max_attempts.to_string())],
        )?;
        Ok(())
    }

    pub fn pop_available(&self) -> Result<Option<Job>> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let now = Utc::now().to_rfc3339();

        if let Some(r) = conn.query_row(
            "SELECT id, task_name, payload, available_at, attempts, max_attempts FROM jobs WHERE available_at <= ?1 ORDER BY available_at ASC LIMIT 1",
            [&now],
            |row| {
                let id: String = row.get(0)?;
                let task_name: String = row.get(1)?;
                let payload: String = row.get(2)?;
                let available_at_str: String = row.get(3)?;
                let attempts_str: String = row.get(4)?;
                let max_attempts_str: String = row.get(5)?;
                Ok((id, task_name, payload, available_at_str, attempts_str, max_attempts_str))
            },
        ).optional()? {
            let (id, task_name, payload_str, available_at_str, attempts_str, max_attempts_str) = r;

            conn.execute("DELETE FROM jobs WHERE id = ?1", [&id])?;

            let available_at = DateTime::parse_from_rfc3339(&available_at_str)?.with_timezone(&Utc);
            let payload = BASE64_ENGINE.decode(&payload_str)
                .map_err(|e| anyhow::anyhow!("Failed to decode base64 payload: {}", e))?;
            let attempts = attempts_str.parse::<u32>()
                .map_err(|e| anyhow::anyhow!("Failed to parse attempts: {}", e))?;
            let max_attempts = max_attempts_str.parse::<u32>()
                .map_err(|e| anyhow::anyhow!("Failed to parse max_attempts: {}", e))?;

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

        let conn = rusqlite::Connection::open(&self.db_path)?;
        let payload_b64 = BASE64_ENGINE.encode(&job.payload);
        conn.execute(
            "INSERT OR REPLACE INTO jobs (id, task_name, payload, available_at, attempts, max_attempts) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            [&job.id, &job.task_name, &payload_b64, &job.available_at.to_rfc3339(), &(job.attempts.to_string()), &(job.max_attempts.to_string())],
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
        assert!(!job.id.is_empty());
    }

    #[test]
    fn test_job_creation_with_delay() {
        let job1 = Job::new("task1", vec![], 0, 1);
        let job2 = Job::new("task2", vec![], 10, 1);
        assert!(job2.available_at > job1.available_at);
    }

    #[test]
    fn test_queue_init() {
        let (queue, _temp_dir) = setup_test_queue();
        // If we get here without error, initialization succeeded
        assert!(!queue.db_path.is_empty());
    }

    #[test]
    fn test_enqueue_single_job() {
        let (queue, _temp_dir) = setup_test_queue();
        let job = Job::new("test_task", b"payload".to_vec(), 0, 1);
        assert!(queue.enqueue(&job).is_ok());
    }

    #[test]
    fn test_enqueue_multiple_jobs() {
        let (queue, _temp_dir) = setup_test_queue();
        for i in 0..5 {
            let job = Job::new(&format!("task_{}", i), vec![i as u8], 0, 1);
            assert!(queue.enqueue(&job).is_ok());
        }
    }

    #[test]
    fn test_pop_available_empty_queue() {
        let (queue, _temp_dir) = setup_test_queue();
        let result = queue.pop_available();
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_pop_available_with_job() {
        let (queue, _temp_dir) = setup_test_queue();
        let original_job = Job::new("test_task", b"test_payload".to_vec(), 0, 1);
        let job_id = original_job.id.clone();

        queue.enqueue(&original_job).expect("Failed to enqueue");
        let popped = queue.pop_available().expect("Failed to pop");

        assert!(popped.is_some());
        let job = popped.unwrap();
        assert_eq!(job.id, job_id);
        assert_eq!(job.task_name, "test_task");
        assert_eq!(job.payload, b"test_payload".to_vec());
    }

    #[test]
    fn test_pop_available_respects_delay() {
        let (queue, _temp_dir) = setup_test_queue();
        // Enqueue a job with 1 hour delay
        let delayed_job = Job::new("delayed_task", vec![], 3600, 1);
        queue.enqueue(&delayed_job).expect("Failed to enqueue");

        // Should not be available yet
        let popped = queue.pop_available().expect("Failed to pop");
        assert!(popped.is_none());
    }

    #[test]
    fn test_retry_job_increments_attempts() {
        let (queue, _temp_dir) = setup_test_queue();
        let mut job = Job::new("test_task", vec![1, 2, 3], 0, 5);
        job.attempts = 2;

        queue.retry_job(job.clone()).expect("Failed to retry");
        let popped = queue.pop_available().expect("Failed to pop");

        assert!(popped.is_some());
        let retried_job = popped.unwrap();
        assert_eq!(retried_job.attempts, 3);
    }

    #[test]
    fn test_retry_job_exponential_backoff() {
        let (queue, _temp_dir) = setup_test_queue();
        let mut job = Job::new("test_task", vec![], 0, 5);
        let _now = Utc::now();

        job.attempts = 0;
        queue.retry_job(job.clone()).expect("Failed to retry");
        let popped = queue.pop_available().expect("Failed to pop");
        assert!(popped.is_none()); // Job scheduled for future

        // Re-enqueue for next test
        queue.enqueue(&job).expect("Failed to enqueue");

        let mut job2 = Job::new("test_task2", vec![], 0, 5);
        job2.attempts = 3;
        queue.retry_job(job2).expect("Failed to retry");

        // Attempt 3 should have 2^3 = 8 second backoff
        let popped2 = queue.pop_available().expect("Failed to pop");
        assert!(popped2.is_none()); // Still in future
    }

    #[test]
    fn test_retry_job_max_attempts() {
        let (queue, _temp_dir) = setup_test_queue();
        let mut job = Job::new("test_task", vec![], 0, 2);
        job.attempts = 2; // Already at max

        queue.retry_job(job.clone()).expect("Failed to retry");
        // Job should be scheduled far in the future (3650 days)

        let popped = queue.pop_available().expect("Failed to pop");
        assert!(popped.is_none()); // Not available until 3650 days pass
    }

    #[test]
    fn test_job_roundtrip_preserves_payload() {
        let (queue, _temp_dir) = setup_test_queue();
        let original_payload = vec![42, 99, 1, 0, 255, 128];
        let job = Job::new("encode_test", original_payload.clone(), 0, 1);

        queue.enqueue(&job).expect("Failed to enqueue");
        let popped = queue.pop_available().expect("Failed to pop");

        let retrieved_job = popped.expect("Job not found");
        assert_eq!(retrieved_job.payload, original_payload);
    }

    #[test]
    fn test_enqueue_preserves_all_fields() {
        let (queue, _temp_dir) = setup_test_queue();
        let original_job = Job::new("complex_task", b"data".to_vec(), 5, 3);
        let job_id = original_job.id.clone();
        let task_name = original_job.task_name.clone();
        let attempts = original_job.attempts;
        let max_attempts = original_job.max_attempts;

        queue.enqueue(&original_job).expect("Failed to enqueue");

        // Need to wait or adjust delay to retrieve
        let later_job = Job::new("later", vec![], 0, 1);
        queue.enqueue(&later_job).expect("Failed to enqueue");

        let popped = queue.pop_available().expect("Failed to pop");
        let retrieved_job = popped.expect("Job not found");

        assert_eq!(retrieved_job.id, job_id);
        assert_eq!(retrieved_job.task_name, task_name);
        assert_eq!(retrieved_job.attempts, attempts);
        assert_eq!(retrieved_job.max_attempts, max_attempts);
    }

    #[test]
    fn test_multiple_jobs_fifo_order() {
        let (queue, _temp_dir) = setup_test_queue();

        for i in 0..3 {
            let job = Job::new(&format!("task_{}", i), vec![i as u8], 0, 1);
            queue.enqueue(&job).expect("Failed to enqueue");
        }

        // Pop should return them in order (earliest available_at first)
        let job1 = queue
            .pop_available()
            .expect("Failed to pop")
            .expect("Job1 missing");
        assert_eq!(job1.task_name, "task_0");

        let job2 = queue
            .pop_available()
            .expect("Failed to pop")
            .expect("Job2 missing");
        assert_eq!(job2.task_name, "task_1");

        let job3 = queue
            .pop_available()
            .expect("Failed to pop")
            .expect("Job3 missing");
        assert_eq!(job3.task_name, "task_2");
    }

    #[test]
    fn test_job_deletion_on_pop() {
        let (queue, _temp_dir) = setup_test_queue();
        let job = Job::new("test_task", vec![], 0, 1);
        let _job_id = job.id.clone();

        queue.enqueue(&job).expect("Failed to enqueue");
        let popped1 = queue.pop_available().expect("Failed to pop");
        assert!(popped1.is_some());

        // Job should be deleted, so popping again returns None
        let popped2 = queue.pop_available().expect("Failed to pop");
        assert!(popped2.is_none());
    }

    #[test]
    fn test_empty_payload() {
        let (queue, _temp_dir) = setup_test_queue();
        let job = Job::new("empty_task", vec![], 0, 1);

        queue.enqueue(&job).expect("Failed to enqueue");
        let popped = queue
            .pop_available()
            .expect("Failed to pop")
            .expect("Job missing");

        assert_eq!(popped.payload, vec![]);
    }
}
