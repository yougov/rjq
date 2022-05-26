extern crate rjq;

use std::thread::sleep;
use std::time::Duration;

use rjq::Queue;

mod errors {
    use rjq::errors::Error as RjqError;

    #[derive(Debug, thiserror::Error)]
    #[non_exhaustive]
    pub enum Error {
        #[error("{0}")]
        Redis(#[from] redis::RedisError),
        #[error("{0}")]
        Serde(#[from] serde_json::Error),
        #[error("{0}")]
        Rjq(#[from] RjqError),
    }
}

type JobResult = rjq::JobResult<errors::Error>;

fn main() {
    fn process(uuid: String, _: Vec<String>) -> JobResult {
        sleep(Duration::from_millis(1000));
        println!("{}", uuid);
        Ok(Some(format!("hi from {}", uuid)))
    }

    let queue = Queue::new("redis://localhost/", "rjq");
    queue
        .work(process, None, Some(5), Some(10), None, Some(false), None)
        .unwrap();
}
