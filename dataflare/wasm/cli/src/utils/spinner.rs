//! Spinner utility for showing progress

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use colored::*;

/// A simple spinner for showing progress
pub struct Spinner {
    message: Arc<Mutex<String>>,
    running: Arc<Mutex<bool>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    /// Create a new spinner with the given message
    pub fn new(message: &str) -> Self {
        Self {
            message: Arc::new(Mutex::new(message.to_string())),
            running: Arc::new(Mutex::new(false)),
            handle: None,
        }
    }

    /// Start the spinner
    pub fn start(&mut self) {
        *self.running.lock().unwrap() = true;
        
        let message = Arc::clone(&self.message);
        let running = Arc::clone(&self.running);
        
        let handle = thread::spawn(move || {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut frame_index = 0;
            
            while *running.lock().unwrap() {
                let current_message = message.lock().unwrap().clone();
                print!("\r{} {}", frames[frame_index].cyan(), current_message);
                io::stdout().flush().unwrap();
                
                frame_index = (frame_index + 1) % frames.len();
                thread::sleep(Duration::from_millis(100));
            }
            
            // Clear the line
            print!("\r{}", " ".repeat(80));
            print!("\r");
            io::stdout().flush().unwrap();
        });
        
        self.handle = Some(handle);
    }

    /// Update the spinner message
    pub fn update(&self, message: &str) {
        *self.message.lock().unwrap() = message.to_string();
    }

    /// Stop the spinner
    pub fn stop(&mut self) {
        *self.running.lock().unwrap() = false;
        
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop();
    }
}
