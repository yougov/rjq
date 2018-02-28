#[macro_use]
extern crate error_chain;
extern crate rjq;

use std::time::Duration;
use std::thread::sleep;
use rjq::Queue;

mod errors {
    extern crate redis;
    extern crate serde_json;
    use rjq::errors as rjqErrors;
    error_chain!{
        foreign_links {
            Redis(redis::RedisError);
            Serde(serde_json::Error);
            Rjq(rjqErrors::Error);
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

    let queue = Queue::new("redis://localhost/", "rjq");
    queue
        .work(process, None, Some(5), Some(10), None, Some(false), None)
        .unwrap();
}
