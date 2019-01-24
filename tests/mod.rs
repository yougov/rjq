#[cfg(test)]
#[macro_use]
extern crate error_chain;
extern crate rjq;

use rjq::{Queue, Status};
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

#[test]
fn test_job_queued() {
    let queue = Queue::new("redis://localhost/", "test-queued").unwrap();
    queue.drop().unwrap();

    let uuid = queue.enqueue(None, vec![], 5).unwrap();

    let status = queue.status(&uuid).unwrap();
    assert!(status == Status::QUEUED);
}

#[test]
#[should_panic]
fn test_job_expired() {
    let queue = Queue::new("redis://localhost/", "test-expired").unwrap();
    queue.drop().unwrap();

    let uuid = queue.enqueue(None, vec![], 1).unwrap();
    sleep(Duration::from_millis(2000));

    queue.status(&uuid).unwrap();
}

#[test]
fn test_job_finished() {
    fn fn_ok(_: String, _: Vec<String>) -> JobResult {
        sleep(Duration::from_millis(1_000));
        Ok(Some("ok".to_string()))
    }

    let queue = Queue::new("redis://localhost/", "test-finished").unwrap();
    queue.drop().unwrap();

    let uuid = queue.enqueue(None, vec![], 10).unwrap();
    queue
        .work(
            fn_ok,
            Some(1),
            Some(5),
            Some(1),
            Some(5),
            Some(false),
            Some(false),
        )
        .unwrap();

    let status = queue.status(&uuid).unwrap();
    assert!(status == Status::FINISHED(Some("ok".to_string())));
}

#[test]
fn test_job_result() {
    fn fn_ok(_: String, _: Vec<String>) -> JobResult {
        sleep(Duration::from_millis(200));
        Ok(Some("ok".to_string()))
    }

    let queue = Queue::new("redis://localhost/", "test-result").unwrap();
    queue.drop().unwrap();

    let uuid = queue.enqueue(None, vec![], 10).unwrap();
    queue
        .work(
            fn_ok,
            Some(1),
            Some(5),
            Some(1),
            Some(5),
            Some(false),
            Some(false),
        )
        .unwrap();

    let res = queue.result(&uuid).unwrap();
    assert!(res == Some("ok".to_string()));
}

#[test]
fn test_job_failed() {
    fn fn_err(_: String, _: Vec<String>) -> JobResult {
        sleep(Duration::from_millis(200));
        Err("err".into())
    }

    let queue = Queue::new("redis://localhost/", "test-failed").unwrap();
    queue.drop().unwrap();

    let uuid = queue.enqueue(None, vec![], 10).unwrap();
    queue
        .work(
            fn_err,
            Some(1),
            Some(5),
            Some(1),
            Some(5),
            Some(false),
            Some(false),
        )
        .unwrap();

    let status = queue.status(&uuid).unwrap();
    assert_eq!(
        status,
        Status::FAILED {
            message: "err".to_string(),
            backtrace: "Error: err\n".to_string(),
        }
    );
}

#[test]
fn test_job_lost() {
    fn fn_ok(_: String, _: Vec<String>) -> JobResult {
        sleep(Duration::from_millis(10_000));
        Ok(Some("ok".to_string()))
    }

    let queue = Queue::new("redis://localhost/", "test-lost").unwrap();
    queue.drop().unwrap();

    let uuid = queue.enqueue(None, vec![], 10).unwrap();
    queue
        .work(
            fn_ok,
            Some(1),
            Some(5),
            Some(1),
            Some(5),
            Some(false),
            Some(false),
        )
        .unwrap();

    let status = queue.status(&uuid).unwrap();
    assert!(status == Status::LOST);
}
