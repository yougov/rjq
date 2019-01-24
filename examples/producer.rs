extern crate rjq;

use rjq::Queue;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let queue = Queue::new("redis://localhost/", "rjq").unwrap();
    let mut uuids = Vec::new();

    for _ in 0..10 {
        sleep(Duration::from_millis(100));
        uuids.push(queue.enqueue(None, vec![], 30).unwrap());
    }

    sleep(Duration::from_millis(10_000));

    for uuid in &uuids {
        let status = queue.status(uuid).unwrap();
        let result = queue.result(uuid).unwrap();
        println!("{} {:?} {}", uuid, status, result.unwrap());
    }
}
