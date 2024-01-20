use std::fs::File;
use std::io::{BufReader, BufRead, Read};

use rayon::prelude::*;

use crate::{
    FRAMES_DIR, Result, Error,
    color::Color, 
    pixel::Pixel
};


#[derive(Debug, Clone)]
pub struct FrameFile {
    idx: usize,
    path: String
}

impl FrameFile {
    pub fn new(idx: usize) -> Self {
        let path = format!("{FRAMES_DIR}/frame{idx}.ppm");
        
        Self { idx, path }
    }
    pub fn idx(&self) -> usize { self.idx }
    
    pub fn load(&self) -> Result<Frame> {
        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);
        
        let mut line = String::new();
        reader.read_line(&mut String::new())?; // skip P6
        reader.read_line(&mut line)?; // width + height        
        
        let mut iter = line.split_whitespace();
        
        let width = iter.next()
            .expect("Unreachable")
            .parse::<usize>()
            .map_err(|e| Error::FileParseError(e.to_string()))?;

        let height = iter.next()
            .ok_or(Error::FileParseError(
                "Unexpected end of line".to_string()
            ))?
            .parse::<usize>()
            .map_err(|e| Error::FileParseError(e.to_string()))?;
        
        reader.read_line(&mut String::new())?; // skip maxval

        let mut data = Vec::new();        
        reader.read_to_end(&mut data)?;

        let data = data.chunks(3)
            .map(|c| Color::new(c[0], c[1], c[2]))
            .collect::<Vec<_>>();

        Ok(Frame { width, height, data: data.into(), })        
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    width: usize,
    height: usize,
    data: Box<[Color]>,
}

impl Frame {
    pub fn debug(width: usize, height: usize) -> Self {
        let data = vec![Color::new(128, 128, 128); width * height].into(); 
        Self { width, height, data }    
    }

    #[inline] pub fn data(&self) -> &[Color] { &self.data }
    #[inline] pub fn data_mut(&mut self) -> &mut [Color] { &mut self.data }  
    #[inline] pub fn width(&self) -> usize { self.width }
    #[inline] pub fn height(&self) -> usize { self.height }  

    pub fn to_full_frame_data(&self) -> FrameData {
        FrameData::Full {
            width: self.width as u16, 
            height: self.height as u16, 
            data: self.data.to_vec() 
        }
    }
    pub fn apply_pixels(&self, pixels: &Vec<Pixel>) -> Self {
        let mut data = self.data.clone();
        for p in pixels {
            let i = p.y * self.width + p.x;
            data[i] = p.color;
        }
        Self {
            width: self.width,
            height: self.height,
            data,
        }
    }
    pub fn apply_frame_data(&self, data: &FrameData) -> Self {
        match data {
            FrameData::Delta(d) => self.apply_pixels(d),
            FrameData::Full { width: w, height: h, data: d } => Self {
                width: *w as usize,
                height: *h as usize,
                data: d.clone().into(),
            },
            FrameData::Empty => self.clone(),
        }
    }

    pub fn to_pixels(&self) -> Vec<Pixel> {
        self.data.into_par_iter()
            .enumerate()
            .map(|(i, v)| {
                let x = i % self.width;
                let y = i / self.width;
                Pixel { x, y, color: *v }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub enum FrameData {
    Delta(Vec<Pixel>),
    Full { width: u16, height: u16, data: Vec<Color> },
    Empty
}
impl FrameData {
    pub fn to_pixels(self) -> Vec<Pixel> {
        match self {
            Self::Delta(d) => d,
            Self::Full { width: w, height: h, data: d } => {
                (0..w as usize * h as usize)
                    .into_par_iter()
                    .map(|i| {
                        let x = i % w as usize;
                        let y = i / w as usize;
                        Pixel { x, y, color: d[i] }
                    })
                    .collect()
            },
            Self::Empty => Vec::new()
        }
    }
}