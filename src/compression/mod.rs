use serde::{Serialize, Deserialize};

mod v1;
mod v2;

use v1::VideoCompressorV1;
use v2::VideoCompressorV2;

use crate::{
    args::CompressionLevelArg, frame::{Frame,FrameData}, Result
};

macro_rules! impl_video_compressor {
    { $($name:ident, $t:ty);*; } => {
        #[derive(Clone)]
        pub enum VideoCompressor {
            $($name($t)),*
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum CompressionAlgConfig {
            $($name),*
        }

        impl VideoCompressor {
            pub fn new(alg: CompressionAlgConfig, level: CompressionLevelArg, debug: bool) -> Result<Self> {        
                match alg {
                    $(CompressionAlgConfig::$name => Ok(Self::$name(<$t>::new(level, debug)?)),)*
                }
            }

            pub fn compress_frame(&mut self, new_frame: &Frame) -> FrameData {
                match self {
                    $(Self::$name(c) => c.compress_frame(new_frame)),*
                }
            }
        }
    };   
}

impl_video_compressor! { 
    V1, VideoCompressorV1; 
    V2, VideoCompressorV2; 
}

impl Default for CompressionAlgConfig {
    fn default() -> Self {
        Self::V2
    }
}