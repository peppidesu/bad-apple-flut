use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

use crate::cache::CacheKey;

#[derive(ClapSerde, Clone, Debug, Serialize, Deserialize)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Input file
    #[clap(short, long)]
    #[serde(skip)]
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
    
    /// Compression level [none|low|medium|high|trash-compactor|number]
    #[clap(long)]
    pub compression: String,
    
    /// Target canvas (if supported)
    #[clap(long, default_value = "0")]
    pub canvas: u8,
    
    /// Ignore frame cache
    #[clap(long, action=clap::ArgAction::SetTrue)]
    pub nocache: bool,

    /// Compress frames just-in-time
    #[clap(long, action=clap::ArgAction::SetTrue)]
    pub jit: bool,

    #[clap(long, action=clap::ArgAction::SetTrue)]
    pub debug: bool,
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

#[derive(Clone, Copy, Debug)]
pub enum CompressionLevelArg {
    None,
    Low,
    Medium,
    High,    
    TrashCompactor,
    Number(usize)
}

impl TryFrom<String> for CompressionLevelArg {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "none" => Ok(Self::None),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "trash-compactor" => Ok(Self::TrashCompactor),
            _ => {
                match value.parse::<usize>() {
                    Ok(n) => Ok(Self::Number(n)),
                    Err(_) => Err("Invalid compression level")
                }
            }
        }
    }
}