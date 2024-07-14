use rayon::prelude::*;

use crate::{
    args::CompressionLevelArg,
    frame::{Frame, FrameData},
    Error, Pixel, Result,
};

#[derive(Clone)]
pub struct VideoCompressorV2 {
    last_frame: Option<Frame>,
    level: CompressionLevelV2,
    debug: bool,
}

impl VideoCompressorV2 {
    pub fn new(level: CompressionLevelArg, debug: bool) -> Result<Self> {
        Ok(Self {
            last_frame: None,
            level: level.try_into()?,                
            debug,
        })
    }

    fn delta(&self, old: &Frame, new: &Frame) -> FrameData {
        let mut priorities = old
            .data()
            .into_par_iter()
            .zip(new.data().into_par_iter())
            .enumerate()
            .flat_map(|(i, (old_val, new_val))| {
                let x = i % old.width();
                let y = i / old.width();

                // temporal chroma subsampling
                let (old_y, old_u, old_v) = old_val.to_cielab();
                let (new_y, new_u, new_v) = new_val.to_cielab();

                // euclidean distance
                let diff = (old_y as i32 - new_y as i32).pow(2) as usize
                    + (old_u as i32 - new_u as i32).pow(2) as usize
                    + (old_v as i32 - new_v as i32).pow(2) as usize;

                if diff > 2 {
                    Some((
                        diff,
                        Pixel {
                            x,
                            y,
                            color: *new_val,
                        },
                    ))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if priorities.len() == 0 {
            FrameData::Empty
        } else {
            priorities.sort_unstable_by(|a, b| b.0.cmp(&a.0));

            let data: Vec<_> = priorities
                .into_iter()
                .take(self.level.target_pixels_per_frame)
                .map(|(_, px)| px)
                .collect();
            FrameData::Delta(data)
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
                } else {
                    data
                }
            }
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
#[derive(Clone)]
pub struct CompressionLevelV2 {
    target_pixels_per_frame: usize,
}

impl CompressionLevelV2 {
    pub fn new(target_pixels_per_frame: usize) -> Self {
        Self {
            target_pixels_per_frame,
        }
    }
}

impl TryFrom<CompressionLevelArg> for CompressionLevelV2 {
    type Error = Error;

    fn try_from(arg: CompressionLevelArg) -> Result<Self> {
        match arg {
            CompressionLevelArg::Number(n) => Ok(Self::new(n)),
            _ => Err(Error::InvalidArgs(
                "Invalid compression level for V2".to_string(),
            )),
        }
    }
}