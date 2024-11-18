//! Redis job queue and worker crate.
//!
//! # Enqueue jobs
//!
//! ```rust
//! use std::time::Duration;
//! use std::thread::sleep;
//!
//! let queue = rjq::Queue::new("redis://localhost/", "rjq", 10).unwrap();
//! let mut uuids = Vec::new();
//!
//! for _ in 0..10 {
//!     sleep(Duration::from_millis(100));
//!     uuids.push(queue.enqueue(None, vec![], 30).unwrap());
//! }
//! sleep(Duration::from_millis(3000));
//!
//! for uuid in uuids.iter() {
//!     let status = queue.status(uuid).unwrap();
//!     let result = queue.result(uuid);
//!     if let Ok(result) = result {
//!         println!("{} {:?} {:?}", uuid, status, result);
//!     } else {
//!         println!("{:?}", result);
//!     }
//! }
//! ```
//!
//! # Work on jobs
//!
//! ```rust
//! use std::time::Duration;
//! use std::thread::sleep;
//!
//! #[derive(Debug, thiserror::Error)]
//! #[non_exhaustive]
//! pub enum Error {
//!     #[error("{0}")]
//!     Redis(#[from] redis::RedisError),
//!     #[error("{0}")]
//!     Serde(#[from] serde_json::Error),
//!     #[error("{0}")]
//!     Rjq(#[from] rjq::errors::Error),
//! }
//!
//! fn process(uuid: String, _: Vec<String>) -> rjq::JobResult<Error> {
//!     sleep(Duration::from_millis(1000));
//!     println!("{}", uuid);
//!     Ok(Some(format!("hi from {}", uuid)))
//! }
//!
//! let queue = rjq::Queue::new("redis://localhost/", "rjq", 10).unwrap();
//! queue.work(process, Some(5.0), Some(10), Some(1), Some(30), Some(false), Some(false)).unwrap();
//! ```

#![allow(clippy::too_many_arguments)]

use std::error::Error as StdError;
use std::fmt::Write;
use std::marker::{Send, Sync};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use redis::{Client, Commands};
use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub use crate::errors::Error;

pub mod errors {
    use redis::RedisError;
    use serde_json::Error as SerdeError;
    use r2d2::Error as R2d2Error;

    /// Crate result.
    pub type Result<T> = std::result::Result<T, Error>;

    /// Crate error.
    #[derive(Debug, thiserror::Error)]
    #[non_exhaustive]
    pub enum Error {
        #[error("Job failed: '{message}', backtrace: {backtrace}")]
        JobFailed {
            message: String,
            backtrace: String,
        },
        #[error("Job queued")]
        JobQueued,
        #[error("Job lost")]
        JobLost,
        #[error("Job running")]
        JobRunning,
        #[error("{0}")]
        R2d2(#[from] R2d2Error),
        #[error("{0}")]
        Redis(#[from] RedisError),
        #[error("{0}")]
        Serde(#[from] SerdeError),
    }
}

/// Return type for the 'process' function; wraps
/// an optional result String and an error type.
pub type JobResult<T> = Result<Option<String>, T>;

/// Job status
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    /// Job is queued
    QUEUED,
    /// Job is running
    RUNNING(Option<String>),
    /// Job was lost - timeout exceeded
    LOST,
    /// Job finished successfully
    FINISHED(Option<String>),
    /// Job failed
    FAILED {
        /// Error message
        message: String,
        /// Error backtrace
        backtrace: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct Job {
    id: String,
    status: Status,
    args: Vec<String>,
}

impl Job {
    fn new(id: Option<String>, args: Vec<String>) -> Job {
        Job {
            id: id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            status: Status::QUEUED,
            args,
        }
    }
}

type RedisPool = r2d2::Pool<Client>;
type RedisConn = r2d2::PooledConnection<Client>;

/// Queue
pub struct Queue<'a> {
    /// Redis url
    pool: RedisPool,
    /// Queue name
    name: &'a str,
}

impl<'a> Queue<'a> {
    /// Return a Redis connection for this Queue
    fn redis_connection(&self) -> errors::Result<RedisConn> {
        Ok(self.pool.get()?)
    }

    /// Init new queue object
    ///
    /// `url` - redis url to connect
    ///
    /// `name` - queue name
    pub fn new(url: &str, name: &'a str, pool_size: u32) -> errors::Result<Queue<'a>> {
        let client = redis::Client::open(url)?;
        let pool = r2d2::Builder::new().max_size(pool_size).build(client)?;
        Ok(Queue { pool, name })
    }

    /// Delete enqueued jobs
    pub fn drop(&self) -> errors::Result<()> {
        let mut conn = self.redis_connection()?;

        conn.del(format!("{}:ids", self.name))?;

        Ok(())
    }

    /// Enqueue new job
    ///
    /// `args` - job arguments
    ///
    /// `expire` - job expiration time in seconds, if hasn't started during this time it will be
    /// removed
    ///
    /// Returns unique job identifier
    pub fn enqueue(
        &self,
        id: Option<String>,
        args: Vec<String>,
        expire: u64,
    ) -> errors::Result<String> {
        let mut conn = self.redis_connection()?;

        let job = Job::new(id, args);

        conn.set_ex(
            format!("{}:{}", self.name, job.id),
            serde_json::to_string(&job)?,
            expire,
        )?;
        conn.rpush(format!("{}:ids", self.name), &job.id)?;

        Ok(job.id)
    }

    /// Get job status
    ///
    /// `id` - unique job identifier
    ///
    /// Returns job status
    pub fn status(&self, id: &str) -> errors::Result<Status> {
        let mut conn = self.redis_connection()?;
        let json: String = conn.get(format!("{}:{}", self.name, id))?;
        let job: Job = serde_json::from_str(&json)?;

        Ok(job.status)
    }

    /// List jobs
    ///
    /// Returns list of jobs in JSON format
    pub fn get_jobs_json(&self) -> errors::Result<serde_json::Value> {
        let mut conn = self.redis_connection()?;
        let keys: Vec<String> = conn.keys(format!("{}:*", self.name))?;

        let jobs: Vec<Job> = keys
            .iter()
            .filter_map(|key| conn.get(key.to_string()).ok())
            .filter_map(|json: String| serde_json::from_str(&json).ok())
            .collect();

        Ok(json!({ "jobs": jobs }))
    }

    /// Work on queue, process enqueued jobs
    ///
    /// `fun` - function that would work on jobs
    ///
    /// `wait` - timeout in seconds to wait for one iteration of BLPOP, 10 by default
    ///
    /// `timeout` - timeout in seconds, if job hasn't been completed during this time, it will be
    /// marked as lost, 30 by default
    ///
    /// `freq` - frequency of checking job status while counting on timeout, number of checks per
    /// second, 1 by default
    ///
    /// `expire` - job result expiration time in seconds, 30 by default
    ///
    /// `fall` - panic if job was lost, true by default
    ///
    /// `infinite` - process jobs infinitely, true by default
    pub fn work<T, F: Fn(String, Vec<String>) -> JobResult<T> + Send + Sync + 'static>(
        &self,
        fun: F,
        wait: Option<f64>,
        timeout: Option<u32>,
        freq: Option<u32>,
        expire: Option<u64>,
        fall: Option<bool>,
        infinite: Option<bool>,
    ) -> Result<(), T>
        where
            T: From<redis::RedisError>
            + From<serde_json::Error>
            + From<errors::Error>
            + StdError
    {
        let wait = wait.unwrap_or(10.0);
        let timeout = timeout.unwrap_or(30);
        let freq = freq.unwrap_or(1);
        let expire = expire.unwrap_or(30);
        let fall = fall.unwrap_or(true);
        let infinite = infinite.unwrap_or(true);

        let mut conn = self.redis_connection()?;

        let afun = Arc::new(fun);
        let ids_key = format!("{}:ids", self.name);
        loop {
            let ids: Vec<String> = conn.blpop(&ids_key, wait)?;
            if ids.len() < 2 {
                if !infinite {
                    break;
                }
                continue;
            }

            let id = &ids[1].to_string();
            let key = format!("{}:{}", self.name, id);
            let json: String = match conn.get(&key) {
                Ok(o) => o,
                Err(_) => {
                    if !infinite {
                        break;
                    }
                    continue;
                }
            };

            let mut job: Job = serde_json::from_str(&json)?;

            job.status = Status::RUNNING(None);
            // Update job in Redis.
            // The job should fail if:
            //   - the timeout is reached
            //   - the worker function fails
            //   - the worker dies unexpectedly (e.g. a panic or host failure)
            // We expire the key in `10 * freq` seconds, and expect the
            // worker to update that expiry time every `freq` seconds
            // while the job is still running, until `timeout` has expired
            // (`10 * freq` is an arbitrary choice made to allow for network
            // failures etc).
            // After the job has finished, we should set the expiry time to
            // `expiry` seconds so clients can check the results.
            let running_expiry = 10 * freq;
            conn.set_ex(&key, serde_json::to_string(&job)?, u64::from(running_expiry))?;

            let (tx, rx) = channel();
            let cafun = afun.clone();
            let cid = id.clone();
            let cargs = job.args.clone();
            // Run the function in a different thread. When it completes (or fails),
            // send the result back through the transmitter.
            thread::spawn(move || {
                let r = match cafun(cid, cargs) {
                    Ok(o) => Status::FINISHED(o),
                    Err(err) => Status::FAILED {
                        message: err.to_string(),
                        backtrace: Self::display_chain(&err),
                    },
                };
                tx.send(r).unwrap_or(())
            });

            // Poll for job completion.
            for _ in 0..(timeout * freq) {
                let status = rx.try_recv().unwrap_or(Status::RUNNING(None));
                job.status = status;
                match job.status {
                    Status::RUNNING(_) => {
                        conn.expire(&key, i64::from(running_expiry))
                            .unwrap_or_else(|e| println!("Could not update expiry time: {}", e));
                    }
                    _ => break,
                }
                sleep(Duration::from_millis(1000 / freq as u64));
            }
            if let Status::RUNNING(_) = job.status {
                job.status = Status::LOST
            }

            conn.set_ex(&key, serde_json::to_string(&job)?, expire)?;

            if fall && job.status == Status::LOST {
                panic!("LOST");
            }

            if !infinite {
                break;
            }
        }

        Ok(())
    }

    /// Get job result
    ///
    /// `id` - unique job identifier
    ///
    /// Returns job result
    pub fn result(&self, id: &str) -> errors::Result<Option<String>> {
        let mut conn = self.redis_connection()?;

        let json: String = conn.get(format!("{}:{}", self.name, id))?;
        let job: Job = serde_json::from_str(&json)?;

        match job.status {
            Status::FINISHED(result) => Ok(result),
            Status::QUEUED => Err(Error::JobQueued),
            Status::FAILED { message, backtrace } => Err(Error::JobFailed { message, backtrace }),
            Status::LOST => Err(Error::JobLost),
            Status::RUNNING(_) => Err(Error::JobRunning),
        }
    }

    /// Build a full error chain string.
    ///
    /// The `write!()` macro should never return an error for a string,
    /// however, handling it here in case things change in the future.
    /// Using this macro to avoid clippy warnings on
    /// `format!(..)` appended to existing `String`
    /// help: consider using `write!` to avoid the extra allocation
    /// help: for further information visit
    /// https://rust-lang.github.io/rust-clippy/master/index.html#format_push_string
    fn display_chain<T: StdError>(error: &T) -> String {
        let mut chain = format!("Error: {}", error);
        let mut current_source = error.source();
        while let Some(ce) = current_source {
            current_source = ce.source();
            if let Err(e) = write!(chain, "\n{:?}", ce) {
                return format!("Failed to build cause chain: {:?}", e);
            }
        }
        chain
    }
}
