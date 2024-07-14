#![feature(io_error_more)]

mod ffmpeg_cli;
mod cache;
mod args;
mod compression;
mod frame;
mod color;
mod pixel;
mod config;
mod protocol;

use colored::Colorize;
pub use ffmpeg_cli::*;
pub use cache::*;
pub use args::*;
pub use compression::*;
pub use frame::*;
pub use color::*;
pub use pixel::*;
pub use config::*;
pub use protocol::*;

pub mod paths;

use std::{fmt::Display, sync::Arc, thread::{self, JoinHandle}};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),    
    FileParseError(String),
    FFmpegError(String),
    InvalidArgs(String),
    InvalidConfig(String),
    Custom(String),
}

pub type Result<T> = core::result::Result<T, Error>;

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {}", e),
            Error::FileParseError(e) => write!(f, "File parse error: {}", e),
            Error::FFmpegError(e) => write!(f, "FFmpeg error: {}", e),
            Error::InvalidArgs(e) => write!(f, "Invalid arguments: {}", e),
            Error::InvalidConfig(e) => write!(f, "Invalid config: {}", e),
            Error::Custom(e) => write!(f, "{}", e),
        }
    }

}


pub fn progress_tracker(counter: Arc<std::sync::atomic::AtomicUsize>, max: usize, descr: String) -> JoinHandle<()> {    
    thread::spawn(move || {
        let last_count = counter.load(std::sync::atomic::Ordering::Relaxed);
        let start = std::time::Instant::now();
        let mut av_rate = 0.1;
        let mut warmup = 0;        
        
        let cursorpos = crossterm::cursor::position().unwrap();

        let print_over_line = |str| {
            crossterm::execute!(std::io::stdout(), crossterm::cursor::MoveTo(0, cursorpos.1)).unwrap();
            // clear
            crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine)).unwrap();
            crossterm::execute!(std::io::stdout(), crossterm::style::Print(str)).unwrap();            
        };        

        loop {
            let count = counter.load(std::sync::atomic::Ordering::Relaxed);
            warmup += 1;
            if count == max {
                println!("\nDone.");
                break;
            }
            if warmup < 5 {
                print_over_line(format!("{} / {} {}", count, max, descr));                
            }
            else {
                let elapsed = start.elapsed().as_secs_f64();
                
                let rate = (count - last_count) as f64 / elapsed;
                av_rate = av_rate * 0.95 + rate * 0.05;
                
                let eta = (max - count) as f64 / av_rate;
    
                let hrs = (eta / 3600.0) as i32;
                let mins = ((eta - hrs as f64 * 3600.0) / 60.0) as i32;
                let secs = (eta - hrs as f64 * 3600.0 - mins as f64 * 60.0) as i32;
                if hrs > 0 {
                    print_over_line(format!(
                        "{} / {} {} | {}", 
                        count, 
                        max, 
                        descr, 
                        format!("ETA: {:}h{:02}m{:02}s", hrs, mins, secs).cyan()
                    ));
                } else if mins > 0 {
                    print_over_line(format!(
                        "{} / {} {} | {}", 
                        count, 
                        max, 
                        descr,                         
                        format!("ETA: {:}m{:02}s", mins, secs).cyan()
                    ));
                } else {
                    print_over_line(format!(
                        "{} / {} {} | {}", 
                        count, 
                        max, 
                        descr, 
                        format!("ETA: {:}s", secs).cyan()
                    ));
                }
            }
    
            
            thread::sleep(std::time::Duration::from_millis(500));
        }
    })
}
