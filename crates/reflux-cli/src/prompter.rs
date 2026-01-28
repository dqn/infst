//! CLI implementation of SearchPrompter for interactive offset search

use reflux_core::SearchPrompter;
use std::io::{self, BufRead, Write};

/// CLI prompter for interactive offset search
pub struct CliPrompter;

impl SearchPrompter for CliPrompter {
    fn prompt_continue(&self, message: &str) {
        print!("{}", message);
        io::stdout().flush().ok();
        let stdin = io::stdin();
        let mut line = String::new();
        stdin.lock().read_line(&mut line).ok();
    }

    fn prompt_number(&self, prompt: &str) -> u32 {
        loop {
            print!("{}", prompt);
            io::stdout().flush().ok();
            let stdin = io::stdin();
            let mut line = String::new();
            if stdin.lock().read_line(&mut line).is_err() {
                eprintln!("Failed to read input, please try again");
                continue;
            }
            match line.trim().parse::<u32>() {
                Ok(n) => return n,
                Err(_) => {
                    eprintln!("Invalid number, please try again");
                }
            }
        }
    }

    fn display_message(&self, message: &str) {
        println!("{}", message);
    }

    fn display_warning(&self, message: &str) {
        eprintln!("{}", message);
    }
}
