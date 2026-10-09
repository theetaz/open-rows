// SPDX-License-Identifier: Apache-2.0
//! Tiny process fixture compiled in a temporary directory by credential tests.
use std::io::Write as _;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("args") => { print!("{}\n", args.get(2).map_or("", String::as_str)); }
        Some("fail") => {
            println!("test-only");
            eprintln!("test-only\npassword=pw extra\ntoken=x more");
            std::process::exit(1);
        }
        Some("block") => {
            if let Some(path) = args.get(2) { let _ = std::fs::write(path, "started"); }
            let _ = std::io::stdout().flush();
            loop { std::thread::park(); }
        }
        Some("stdin") => {
            let mut text = String::new();
            if std::io::stdin().read_line(&mut text).is_ok_and(|read| read == 0) { println!("pw"); }
        }
        _ => { println!("pw"); }
    }
}
