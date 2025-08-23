use std::io::{self, BufRead};
use std::thread;
use std::time::Duration;

fn main() {
    println!("Plugin started");

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    loop {
        match lines.next() {
            Some(Ok(line)) => {
                println!("Received: {}", line);

                thread::sleep(Duration::from_millis(1000));

                if line == "ping" {
                    println!("pong");
                } else {
                    println!("Echo: {}", line);
                }
            }
            Some(Err(e)) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
            None => {
                // EOF - stdin closed
                println!("Input closed");
                break;
            }
        }
    }
}
