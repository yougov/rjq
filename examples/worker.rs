#[macro_use]
extern crate error_chain;
extern crate rjq;

use rjq::Queue;
use std::thread::sleep;
use std::time::Duration;

mod errors {
    extern crate redis;
    extern crate serde_json;
    use rjq::errors::Error as RjqError;
    error_chain! {
        foreign_links {
            Redis(redis::RedisError);
            Serde(serde_json::Error);
            Rjq(RjqError);
        }
    }
}

type JobResult = rjq::JobResult<errors::Error>;

fn main() {
    fn process(uuid: String, _: Vec<String>) -> JobResult {
        sleep(Duration::from_millis(1000));
        println!("{}", uuid);
        Ok(Some(format!("hi from {}", uuid)))
    }

    let queue: Queue = Queue::new("redis://localhost/", "rjq", 10).unwrap();
    queue
        .work(process, None, Some(5), Some(10), None, Some(false), None)
        .unwrap();
}
