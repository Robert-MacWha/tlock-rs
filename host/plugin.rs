// plugin.rs - simplified
use std::io::{self, BufRead, Write};

fn main() {
    println!("Plugin started");

    let stdin = io::stdin();
    let stdout = io::stdout();

    let reader = stdin.lock();
    let mut lines_iter = reader.lines();

    while let Some(line_result) = lines_iter.next() {
        println!("Plugin: Got Some from iterator");

        if line_result.is_ok() {
            println!("Plugin: line_result is Ok");
        }

        if line_result.is_err() {
            println!("Plugin: line_result is Err");
            break;
        }

        let line = line_result.unwrap();

        println!("Plugin: About to reverse: {}", line);
        let reversed: String = line.chars().rev().collect();

        println!("{}", reversed);
        io::stdout().flush().unwrap();
    }

    println!("Plugin: Iterator returned None, exiting");
}
