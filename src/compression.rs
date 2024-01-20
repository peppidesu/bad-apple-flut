use rayon::prelude::*;

use crate::{
    frame::{Frame,FrameData}, 
    pixel::Pixel, 
    args::CompressionLevelArg
};

pub trait VideoCompressor {
    fn compress_frame(&mut self, new_frame: &Frame) -> FrameData;
}

pub struct DeltaCompressorV1 {
    last_frame: Option<Frame>,
    level: CompressionLevelV1,
    debug: bool,
}

impl DeltaCompressorV1 {
    pub fn new(level: CompressionLevelV1, debug: bool) -> Self {
        Self { last_frame: None, level: level, debug }
    }

    pub fn delta(&self, old: &Frame, new: &Frame) -> FrameData {                
        let px_vec: Vec<_> = old.data().into_par_iter()
            .zip(new.data().into_par_iter())
            .enumerate()
            .filter_map(|(i, (old_val, new_val))| {    
                // temporal chroma subsampling
                let (old_y, old_u, old_v) = old_val.to_yuv();
                let (new_y, new_u, new_v) = new_val.to_yuv();
                
                let y_diff = old_y.abs_diff(new_y) as u16;
                let c_diff = old_u.abs_diff(new_u) as u16 + old_v.abs_diff(new_v) as u16;

                if y_diff > self.level.luminance_treshold()
                || c_diff > self.level.chroma_threshold(old_y) {
                    let x = i % old.width();
                    let y = i / old.width();
                    
                    Some(Pixel { x, y, color: *new_val })
                } else {
                    None
                }
            })
            .collect();
    
        if px_vec.len() == 0 {
            FrameData::Empty
        } else {
            FrameData::Delta(px_vec)
        }
    }

    pub fn compress_frame(&mut self, new_frame: &Frame) -> FrameData {        
        let frame_data = match &self.last_frame {
            Some(lf) => {
                let data = self.delta(&lf, &new_frame);                

                self.last_frame = Some(lf.apply_frame_data(&data));

                if self.debug {
                    let debug_frame = Frame::debug(new_frame.width(), new_frame.height());
                    let debug_frame = debug_frame.apply_frame_data(&data);
                    debug_frame.to_full_frame_data()
                }
                else {
                    data
                }
            },
            None => {
                let data = new_frame.to_full_frame_data();
                self.last_frame = Some(new_frame.clone());
                data
            }
        };
        frame_data
    }
}

///////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevelV1 {    
    None,
    Low,
    Medium,
    High,
    TrashCompactor
}

impl CompressionLevelV1 {
    pub fn luminance_treshold(&self) -> u16 {
        match self {            
            Self::None => 0,
            Self::Low => 3,
            Self::Medium => 7,
            Self::High => 15,
            Self::TrashCompactor => 32,
        }
    }
    pub fn chroma_threshold(&self, y: u8) -> u16 {
        let y = y as f32 / 255.0;
        let t = match self {        
            Self::None => 0.0,
            Self::Low => (1.0 - y) * 8.0,
            Self::Medium => (1.0 - y.powi(2) * 0.85) * 16.0,
            Self::High => (1.0 - y.powi(2) * 0.75) * 32.0,
            Self::TrashCompactor => 64.0,
        };
        t as u16
    }
}

impl From<CompressionLevelArg> for CompressionLevelV1 {
    fn from(arg: CompressionLevelArg) -> Self {
        match arg {
            CompressionLevelArg::None => Self::None,
            CompressionLevelArg::Low => Self::Low,
            CompressionLevelArg::Medium => Self::Medium,
            CompressionLevelArg::High => Self::High,
            CompressionLevelArg::TrashCompactor => Self::TrashCompactor,
        }
    }
}

