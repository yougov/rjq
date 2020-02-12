# Redis Job Queue

**This is a fork of the [original unmaintained repo](https://github.com/embali/rjq).**

Simple redis job queue

[![crates.io](https://img.shields.io/crates/v/rjq.svg)](https://crates.io/crates/rjq)
[![Build Status](https://travis-ci.org/yougov/rjq.svg?branch=dev)](https://travis-ci.org/yougov/rjq)


## Documentation

https://docs.rs/rjq/


## Enqueue jobs

```rust
extern crate rjq;

use std::time::Duration;
use std::thread::sleep;
use rjq::Queue;

fn main() {
    let queue = Queue::new("redis://localhost/", "rjq", 10);
    let mut uuids = Vec::new();

    for _ in 0..10 {
        sleep(Duration::from_millis(100));
        uuids.push(queue.enqueue(vec![], 30).unwrap());
    }

    sleep(Duration::from_millis(10000));

    for uuid in uuids.iter() {
        let status = queue.status(uuid).unwrap();
        let result = queue.result(uuid).unwrap().unwrap();
        println!("{} {:?} {}", uuid, status, result);
    }
}
```


## Queue worker

```rust
extern crate rjq;

use std::time::Duration;
use std::thread::sleep;
use std::error::Error;
use rjq::Queue;

fn main() {
    fn process(uuid: String, _: Vec<String>) -> Result<String, Box<Error>> {
        sleep(Duration::from_millis(1000));
        println!("{}", uuid);
        Ok(format!("hi from {}", uuid))
    }

    let queue = Queue::new("redis://localhost/", "rjq", 10);
    queue.work(process, Some(1), Some(5), Some(10), Some(30), Some(false), None).unwrap();
}
```


## Job status

**QUEUED** - job queued for further processing

**RUNNING** - job is running by worker

**LOST** - job has not been finished in time

**FINISHED** - job has been successfully finished

**FAILED** - job has been failed due to some errors


## Queue methods

### Init queue

```rust
fn new(url: &str, name: &str) -> Queue;
```

**url** - redis URL

**name** - queue name

Returns **queue**

### Drop queue jobs

```rust
fn drop(&self) -> Result<(), Box<Error>>;
```

### Enqueue job

```rust
fn enqueue(&self, args: Vec<String>, expire: usize) -> Result<String, Box<Error>>;
```

**args** - job arguments

**expire** - if job has not been started by worker in this time (in seconds), it will expire

Returns job **UUID**

### Get job status

```rust
fn status(&self, uuid: &str) -> Result<Status, Box<Error>>;
```

**uuid** - job unique identifier

Returns job **status**

### Work on queue

```rust
fn work<F: Fn(String, Vec<String>) -> Result<String, Box<Error>> + Send + Sync + 'static>
    (&self,
     fun: F,
     wait: Option<usize>,
     timeout: Option<usize>,
     freq: Option<usize>,
     expire: Option<usize>,
     fall: Option<bool>,
     infinite: Option<bool>)
     -> Result<(), Box<Error>>;
```

**fun** - worker function

**wait** - time to wait until next job will pop

**timeout** - worker function should finish in timeout (in seconds)

**freq** - job status check frequency (times per second)

**expire** - job result will expire in this time (in seconds)

**fall** - panics to terminate process if the job has been lost

**infinite** - process jobs infinitely one after another, otherwise only one job will be processed

### Get job result

```rust
fn result(&self, uuid: &str) -> Result<Option<String>, Box<Error>>;
```

**uuid** - job unique identifier

Returns job **result**


## Run tests

```bash
cargo test
```
