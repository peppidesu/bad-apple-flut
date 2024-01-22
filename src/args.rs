use clap::{Parser, ValueEnum};

use crate::cache::CacheKey;

#[derive(Parser, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Input file
    #[clap(short, long)]
    pub input: String,

    /// Horizontal offset (in px)
    #[clap(short, default_value = "0")]
    pub x_offset: usize,

    /// Vertical offset (in px)
    #[clap(short, default_value = "0")]
    pub y_offset: usize,
    
    /// Width (in px) [default: same as source]
    #[clap(long)]
    pub width: Option<i32>,

    /// Height (in px) [default: same as source]
    #[clap(long)]
    pub height: Option<i32>,

    /// Frame-rate (in fps) [default: same as source]
    #[clap(long)]
    pub fps: Option<f64>,
    
    /// Compression level
    #[clap(long, default_value = "medium")]
    pub compression: CompressionLevelArg,
    
    /// Ignore frame cache
    #[clap(long)]
    pub nocache: bool,

    /// Compress frames just-in-time
    #[clap(long)]
    pub jit: bool,

    #[clap(long)]
    pub debug: bool
}

impl Args {
    pub fn parse() -> Self {
        Self::parse_from(std::env::args())
    }
}

impl From<Args> for CacheKey {
    fn from(args: Args) -> Self {
        Self::new(
            args.input,
            args.width.unwrap_or(0),
            args.height.unwrap_or(0),
            args.fps.unwrap_or(0.0)
        )
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum CompressionLevelArg {    
    None,
    Low,
    Medium,
    High,    
    TrashCompactor
}