
mod ffmpeg_cli;
mod cache;
mod args;
mod compression;
mod frame;
mod color;
mod pixel;
mod config;

pub use ffmpeg_cli::*;
pub use cache::*;
pub use args::*;
pub use compression::*;
pub use frame::*;
pub use color::*;
pub use pixel::*;
pub use config::*;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    FileParseError(String),
    FFmpegError(String),
}

pub type Result<T> = core::result::Result<T, Error>;

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

pub const HOST: &str = "pixelflut.uwu.industries:1234";
pub const FRAMES_DIR: &str = "cache/frames";
pub const CACHE_ID_PATH: &str = "cache/cache_id";
pub const THREAD_COUNT: usize = 12;
