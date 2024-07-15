use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

use crate::{cache::CacheKey, CompressionAlgConfig, Protocol};

#[derive(ClapSerde, Clone, Debug, Serialize, Deserialize)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Input file
    #[clap(short, long)]
    #[serde(skip)]
    pub input: String,
    
    /// Target section from config file to use
    #[clap(long)]
    #[serde(skip_serializing)]
    pub target: Option<String>,

    /// Host to connect to
    #[clap(long)]
    #[serde(skip_serializing)]    
    pub host: Option<String>,    
    
    /// Protocol to use for sending frames
    #[clap(long)]
    #[serde(default)]
    pub protocol: Protocol,
    
    /// Target canvas (if supported)
    #[clap(long)]
    #[serde(default)]
    pub canvas: u8,

    /// Horizontal offset (in px)
    #[clap(short)]  
    #[serde(default)]  
    pub x_offset: usize,
    
    /// Vertical offset (in px)
    #[clap(short)]
    #[serde(default)]
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
    
    /// Number of threads to use for sending pixels
    #[clap(long)]    
    pub send_threads: usize,    

        /// Number of threads to use for compressing frames
        #[clap(long)]
        pub compress_threads: usize,
    
    /// Compression algorithm to use
    #[clap(long)]
    pub compression_algorithm: CompressionAlgConfig,
    
    /// Compression level [none|low|medium|high|trash-compactor|number]
    #[clap(long)]
    pub compression_level: String,

    /// Number of frames to group together when compressing ahead-of-time
    #[clap(long)]    
    pub aot_frame_group_size: usize,

    /// Ignore frame cache
    #[clap(long, action=clap::ArgAction::SetTrue)]
    #[serde(default)]
    pub nocache: bool,
    
    /// Compress frames just-in-time
    #[clap(long, action=clap::ArgAction::SetTrue)]
    #[serde(default)]
    pub jit: bool,
    
    /// Enable debug output
    #[clap(long, action=clap::ArgAction::SetTrue)]
    #[serde(default)]
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

impl Args {
    pub fn config_default() -> Self {
        Self {
            input: "".to_string(), // will be skipped by serde
            host: None,
            target: None,
            x_offset: 0,
            y_offset: 0,
            width: None,
            height: None,
            fps: None,
            protocol: Protocol::default(),
            canvas: 0,
            nocache: false,
            jit: false,
            debug: false,
            send_threads: 4,
            aot_frame_group_size: 100,
            compression_algorithm: CompressionAlgConfig::V2,
            compression_level: "768".to_string(),
            compress_threads: 4,
        }
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