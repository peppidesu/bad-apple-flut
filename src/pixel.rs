use std::fmt::Write;

use crate::Color;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Pixel {
    pub x: usize,
    pub y: usize,
    pub color: Color,
}

impl Pixel {    
    pub fn to_pixelflut_string(&self, offset_x: usize, offset_y: usize) -> String {        
        let mut result = String::with_capacity(20);
        self.to_pixelflut_writer(offset_x, offset_y, &mut result);
        result
    }

    #[inline]
    pub fn to_pixelflut_writer(&self, offset_x: usize, offset_y: usize, writer: &mut String) {
        let x = self.x + offset_x;
        let y = self.y + offset_y;

        write!(writer, "PX ").unwrap();
        write!(writer, "{} ", x).unwrap();
        write!(writer, "{} ", y).unwrap();
        write!(writer, "{:02x}{:02x}{:02x}\n", self.color.r, self.color.g, self.color.b).unwrap();        
    }
}

#[inline]
pub fn pixels_to_string(pixels: &[Pixel], offset_x: usize, offset_y: usize) -> String {
    let mut result = String::with_capacity(pixels.len() * 20);
    for pixel in pixels {
        pixel.to_pixelflut_writer(offset_x, offset_y, &mut result);
    }
    result
}

