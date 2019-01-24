extern crate rjq;

use rjq::Queue;

fn main() {
    let url = "redis://localhost/";
    let qname = "queue-name";
    let queue = Queue::new(url, qname).unwrap();
    println!("{:?}", queue.get_jobs_json());
}
