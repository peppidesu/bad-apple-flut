use clap::{Parser, ValueEnum};

use crate::cache::CacheKey;

#[derive(Parser, Clone)]
pub struct Args {
    #[clap(short, long)]
    pub input: String,

    #[clap(short)]
    pub x_offset: Option<usize>,

    #[clap(short)]
    pub y_offset: Option<usize>,
    
    #[clap(long)]
    pub width: Option<i32>,
    #[clap(long)]
    pub height: Option<i32>,

    #[clap(long)]
    pub fps: Option<f64>,
    
    #[clap(long)]
    pub nocache: bool,

    #[clap(long, default_value = "medium")]
    pub compression: CompressionLevelArg,

    #[clap(long)]
    pub debug: bool,
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